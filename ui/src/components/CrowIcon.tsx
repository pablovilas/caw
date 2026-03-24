interface CrowIconProps {
  size?: number;
  className?: string;
}

export function CrowIcon({ size = 20, className }: CrowIconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 52 52"
      fill="none"
      className={className}
    >
      <path
        d="M14 34 C14 34 16 20 26 16 C22 20 22 26 26 26 C30 26 34 22 36 16 C38 22 36 30 28 34 L32 38 L26 36 L20 40 L22 34 Z"
        fill="currentColor"
        opacity="0.95"
      />
      <circle cx="30" cy="20" r="1.5" fill="var(--bg-base, #0F0F0F)" />
      <path
        d="M36 16 L40 14 L38 18"
        fill="currentColor"
        opacity="0.6"
      />
    </svg>
  );
}
