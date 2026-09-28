import type { CSSProperties } from 'react';
import chevron from '../assets/icons/chevron.svg';
import copy from '../assets/icons/copy.svg';
import dots from '../assets/icons/dots.svg';
import download from '../assets/icons/download.svg';
import globe from '../assets/icons/globe.svg';
import home from '../assets/icons/home.svg';
import info from '../assets/icons/info.svg';
import plus from '../assets/icons/plus.svg';
import power from '../assets/icons/power.svg';
import refresh from '../assets/icons/refresh.svg';
import rules from '../assets/icons/rules.svg';
import server from '../assets/icons/server.svg';
import settings from '../assets/icons/settings.svg';
import trash from '../assets/icons/trash.svg';
import upload from '../assets/icons/upload.svg';
import winClose from '../assets/icons/win-close.svg';
import winMin from '../assets/icons/win-min.svg';

const ICONS = { chevron, copy, dots, download, globe, home, info, plus, power, refresh, rules, server, settings, trash, upload, winClose, winMin };
export type IconName = keyof typeof ICONS;

/** Иконка из макета: SVG как маска, цвет берётся из currentColor. */
export function Icon({ name, size = 16, className }: { name: IconName; size?: number; className?: string }) {
  const style = { '--src': `url("${ICONS[name]}")`, width: size, height: size } as CSSProperties;
  return <span className={className ? `ico ${className}` : 'ico'} style={style} aria-hidden="true" />;
}
