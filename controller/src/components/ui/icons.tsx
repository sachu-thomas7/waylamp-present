// Shared icon set — one stroke weight (1.6-1.8), one 20px grid, matching the approved design.
import type { SVGProps } from 'react';

type IconProps = SVGProps<SVGSVGElement>;

export function IconPlay(props: IconProps) {
  return (
    <svg width="14" height="14" viewBox="0 0 20 20" fill="none" {...props}>
      <path d="M7 4.5v11l8-5.5-8-5.5z" fill="currentColor" />
    </svg>
  );
}

export function IconStop(props: IconProps) {
  return (
    <svg width="12" height="12" viewBox="0 0 20 20" fill="none" {...props}>
      <rect x="5" y="5" width="10" height="10" rx="2" fill="currentColor" />
    </svg>
  );
}

export function IconReset(props: IconProps) {
  return (
    <svg width="13" height="13" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M15.5 10a5.5 5.5 0 1 1-1.64-3.92" />
      <path d="M15.5 4.5v3.5h-3.5" />
    </svg>
  );
}

export function IconUpload(props: IconProps) {
  return (
    <svg width="13" height="13" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M10 13V4" />
      <path d="M6 8l4-4 4 4" />
      <path d="M4.5 15.5h11" />
    </svg>
  );
}

export function IconImage(props: IconProps) {
  return (
    <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <rect x="3" y="4" width="14" height="12" rx="1.5" />
      <circle cx="7.5" cy="8.5" r="1.1" fill="currentColor" stroke="none" />
      <path d="M3.5 14.5l4.5-4.5 3 3 2.5-2.5 3.5 3.5" />
    </svg>
  );
}

export function IconVideo(props: IconProps) {
  return (
    <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <rect x="3" y="5.5" width="10" height="9" rx="1.5" />
      <path d="M13 8.5l4-2.3v7.6l-4-2.3z" />
    </svg>
  );
}

export function IconMonitor(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <rect x="2.5" y="4" width="15" height="10" rx="1.5" />
      <path d="M7 17h6" />
      <path d="M10 14v3" />
    </svg>
  );
}

export function IconChevronLeft(props: IconProps) {
  return (
    <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M12.5 4.5l-6 5.5 6 5.5" />
    </svg>
  );
}

export function IconChevronRight(props: IconProps) {
  return (
    <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <path d="M7.5 4.5l6 5.5-6 5.5" />
    </svg>
  );
}

export function IconSettings(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <circle cx="10" cy="10" r="2.6" />
      <path d="M10 2.5v2M10 15.5v2M17.5 10h-2M4.5 10h-2M15.1 4.9l-1.4 1.4M6.3 13.7l-1.4 1.4M15.1 15.1l-1.4-1.4M6.3 6.3L4.9 4.9" />
    </svg>
  );
}

export function IconHelp(props: IconProps) {
  return (
    <svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" {...props}>
      <circle cx="10" cy="10" r="7.5" />
      <path d="M7.8 7.8a2.2 2.2 0 1 1 3.3 1.9c-.7.4-1.1.9-1.1 1.8v.3" />
    </svg>
  );
}

export function IconCheck(props: IconProps) {
  return (
    <svg width="13" height="13" viewBox="0 0 20 20" fill="none" {...props}>
      <circle cx="10" cy="10" r="8" fill="currentColor" opacity="0.2" />
      <path d="M6.5 10.2l2.3 2.3 4.7-5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" fill="none" />
    </svg>
  );
}

export function IconSearch(props: IconProps) {
  return (
    <svg width="12" height="12" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" {...props}>
      <circle cx="8.5" cy="8.5" r="5.5" />
      <path d="M16 16l-4-4" />
    </svg>
  );
}

export function IconWarning(props: IconProps) {
  return (
    <svg width="15" height="15" viewBox="0 0 20 20" fill="none" {...props}>
      <path d="M10 3l8 14H2l8-14z" fill="currentColor" />
      <rect x="9.25" y="8" width="1.5" height="4.2" rx="0.75" fill="#1a1206" />
      <rect x="9.25" y="13" width="1.5" height="1.5" rx="0.75" fill="#1a1206" />
    </svg>
  );
}
