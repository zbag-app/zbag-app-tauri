import logoSvg from '../../assets/bagZ_Logo.svg';

interface LogoProps {
  size?: number;
  className?: string;
}

/**
 * bagZ brand logo component.
 * Uses the official SVG from bagz-ux brand assets.
 */
export function Logo({ size = 40, className = '' }: LogoProps) {
  return (
    <img
      src={logoSvg}
      alt="bagZ"
      width={size}
      height={size}
      className={className}
      style={{ width: size, height: size }}
    />
  );
}
