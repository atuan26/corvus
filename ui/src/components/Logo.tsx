import { useId } from "react";

const RAVEN =
  "M232 612c60-150 190-250 340-262 70-6 128 10 170 40l78-34-34 92c22 46 22 104-6 160-56 112-196 168-336 140-58-12-112-40-152-80l-96 30z";

/** The raven mark. Its feather sheen is the one decorative gradient in the app. */
export function LogoMark({ size = 26 }: { size?: number }) {
  const id = useId();
  return (
    <svg width={size} height={size} viewBox="180 300 720 460" aria-hidden className="logo-mark">
      <defs>
        <linearGradient id={id} x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="var(--sheen-1)" />
          <stop offset="0.55" stopColor="var(--sheen-2)" />
          <stop offset="1" stopColor="var(--sheen-3)" />
        </linearGradient>
      </defs>
      <path d={RAVEN} fill={`url(#${id})`} />
      <circle cx="676" cy="430" r="26" fill="var(--bg)" />
    </svg>
  );
}

export function Logo() {
  return (
    <span className="logo">
      <LogoMark />
      <span className="logo-word">Corvus</span>
    </span>
  );
}
