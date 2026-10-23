import AVFoundation
import CoreImage
import PhotosUI
import UIKit

/// Сканер QR-кода подписки для движка (Rust, crates/klick-ios/src/host.rs): камера, а если QR прислали
/// картинкой — из «Фото». Ждёт, пока человек отсканирует или закроет; звать не с главного потока.
/// Строка — через strdup, освобождает Rust; NULL — закрыли без кода.
@_cdecl("klick_scan_qr")
func klickScanQr() -> UnsafeMutablePointer<CChar>? {
    let done = DispatchSemaphore(value: 0)
    var text: String?
    DispatchQueue.main.async {
        guard let top = topController() else {
            done.signal()
            return
        }
        let scanner = QRScanner { result in
            text = result
            done.signal()
        }
        scanner.modalPresentationStyle = .fullScreen
        top.present(scanner, animated: true)
    }
    done.wait()
    return text.flatMap { strdup($0) }
}

private func topController() -> UIViewController? {
    let windows = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows)
    var top = (windows.first(where: \.isKeyWindow) ?? windows.first)?.rootViewController
    while let presented = top?.presentedViewController {
        top = presented
    }
    return top
}

final class QRScanner: UIViewController, AVCaptureMetadataOutputObjectsDelegate, PHPickerViewControllerDelegate {
    private let session = AVCaptureSession()
    private var preview: AVCaptureVideoPreviewLayer?
    private let hint = UILabel()
    private var finish: ((String?) -> Void)?

    init(finish: @escaping (String?) -> Void) {
        self.finish = finish
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        hint.text = "Наведите камеру на QR-код подписки"
        hint.textColor = .white
        hint.textAlignment = .center
        hint.numberOfLines = 0
        hint.font = .preferredFont(forTextStyle: .body)
        let buttons = UIStackView(arrangedSubviews: [button("Из фото", #selector(photoTapped)), button("Отмена", #selector(cancelTapped))])
        buttons.distribution = .fillEqually
        buttons.spacing = 12
        let panel = UIStackView(arrangedSubviews: [hint, buttons])
        panel.axis = .vertical
        panel.spacing = 16
        panel.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(panel)
        NSLayoutConstraint.activate([
            panel.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 20),
            panel.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -20),
            panel.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -20),
        ])
        startCamera()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    private func button(_ title: String, _ action: Selector) -> UIButton {
        let b = UIButton(type: .system)
        b.setTitle(title, for: .normal)
        b.setTitleColor(.white, for: .normal)
        b.titleLabel?.font = .preferredFont(forTextStyle: .headline)
        b.backgroundColor = UIColor(white: 1, alpha: 0.18)
        b.layer.cornerRadius = 12
        b.heightAnchor.constraint(equalToConstant: 48).isActive = true
        b.addTarget(self, action: action, for: .touchUpInside)
        return b
    }

    private func startCamera() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureCamera()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { ok in
                DispatchQueue.main.async { ok ? self.configureCamera() : self.noCamera() }
            }
        default:
            noCamera()
        }
    }

    private func configureCamera() {
        let output = AVCaptureMetadataOutput()
        guard let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input), session.canAddOutput(output)
        else { return noCamera() }
        session.addInput(input)
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.insertSublayer(layer, at: 0)
        preview = layer
        DispatchQueue.global(qos: .userInitiated).async { self.session.startRunning() }
    }

    private func noCamera() {
        hint.text = "Камера недоступна. Разрешите её kl!ck в Настройках или выберите QR-код из фото."
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard let text = (objects.first as? AVMetadataMachineReadableCodeObject)?.stringValue, !text.isEmpty else { return }
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        close(text)
    }

    @objc private func photoTapped() {
        var config = PHPickerConfiguration()
        config.filter = .images
        let picker = PHPickerViewController(configuration: config)
        picker.delegate = self
        present(picker, animated: true)
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        guard let item = results.first?.itemProvider, item.canLoadObject(ofClass: UIImage.self) else { return }
        item.loadObject(ofClass: UIImage.self) { object, _ in
            let text = (object as? UIImage).flatMap(Self.decode)
            DispatchQueue.main.async {
                if let text { self.close(text) } else { self.hint.text = "На картинке не нашлось QR-кода." }
            }
        }
    }

    private static func decode(_ image: UIImage) -> String? {
        guard let picture = CIImage(image: image),
              let detector = CIDetector(ofType: CIDetectorTypeQRCode, context: nil, options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])
        else { return nil }
        return detector.features(in: picture).compactMap { ($0 as? CIQRCodeFeature)?.messageString }.first { !$0.isEmpty }
    }

    @objc private func cancelTapped() {
        close(nil)
    }

    /// Один раз: камера может найти код ещё раз, пока экран закрывается.
    private func close(_ text: String?) {
        guard let finish else { return }
        self.finish = nil
        let session = session
        DispatchQueue.global(qos: .userInitiated).async { session.stopRunning() }
        dismiss(animated: true) { finish(text) }
    }
}
