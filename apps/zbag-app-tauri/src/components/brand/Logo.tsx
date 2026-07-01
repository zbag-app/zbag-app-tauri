import logoSvg from '../../assets/zbag_Logo.svg';

interface LogoProps {
  size?: number;
  className?: string;
}

/**
 * zbag brand logo component.
 */
export function Logo({ size = 40, className = '' }: LogoProps) {
  return (
    <img
      src={logoSvg}
      alt="zbag"
      width={size}
      height={size}
      className={className}
      style={{ width: size, height: size }}
    />
  );
}
