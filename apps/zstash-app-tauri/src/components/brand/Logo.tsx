import logoSvg from '../../assets/zSTASH_Logo.svg';

interface LogoProps {
  size?: number;
  className?: string;
}

/**
 * zSTASH brand logo component.
 * Uses the official SVG from zstash-ux brand assets.
 */
export function Logo({ size = 40, className = '' }: LogoProps) {
  return (
    <img
      src={logoSvg}
      alt="zSTASH"
      width={size}
      height={size}
      className={className}
      style={{ width: size, height: size }}
    />
  );
}
