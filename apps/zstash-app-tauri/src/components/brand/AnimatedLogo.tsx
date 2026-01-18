import { motion } from "framer-motion";

interface AnimatedLogoProps {
  className?: string;
  size?: number;
}

// Logo colors from the brand (top to bottom visual, animation bottom to top)
const LOGO_COLORS = {
  green: "#00ff00", // lime - top layer
  yellow: "#ffff00", // yellow
  orange: "#ff8000", // orange
  red: "#ff0000", // red - bottom layer
  frame: "#f5f5f7", // frame color
};

/**
 * Animated zSTASH logo with spring-physics drop animation.
 * Layers drop from above, starting with red (bottom) and ending with green (top).
 * Metaphor: Funds start "red hot" (not private) and "cool off" to green (fully private).
 */
export function AnimatedLogo({
  className = "",
  size = 140,
}: AnimatedLogoProps) {
  const viewBox = "0 0 1024 1024";

  // Drop animation with spring physics for bounce on landing
  const dropTransition = (delay: number) => ({
    delay,
    type: "spring" as const,
    stiffness: 300,
    damping: 20,
  });

  return (
    <motion.svg
      width={size}
      height={size}
      viewBox={viewBox}
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.2 }}
    >
      {/* Frame - fades in during the stacking sequence */}
      <motion.polygon
        points="955.74 750.94 512 972.8 68.26 750.94 68.26 307.2 136.53 273.06 136.53 716.8 512 904.53 887.47 716.8 887.47 273.06 955.74 307.2 955.74 750.94"
        fill={LOGO_COLORS.frame}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{
          duration: 0.6,
          delay: 0.3,
          ease: [0.0, 0.0, 0.2, 1],
        }}
      />

      {/* Red layer - BOTTOM, drops FIRST (hot, not private) */}
      <motion.polygon
        points="204.8 614.4 204.8 682.67 512 836.27 819.2 682.67 819.2 614.4 512 768 204.8 614.4"
        fill={LOGO_COLORS.red}
        initial={{ opacity: 0, y: -60 }}
        animate={{ opacity: 1, y: 0 }}
        transition={dropTransition(0.2)}
      />

      {/* Orange layer - drops second */}
      <motion.polygon
        points="819.2 546.13 819.2 477.87 512 631.46 204.8 477.87 204.8 546.13 512 699.73 819.2 546.13"
        fill={LOGO_COLORS.orange}
        initial={{ opacity: 0, y: -60 }}
        animate={{ opacity: 1, y: 0 }}
        transition={dropTransition(0.4)}
      />

      {/* Yellow layer - drops third */}
      <motion.polygon
        points="819.2 341.33 512 494.94 204.8 341.33 204.8 409.6 512 563.21 819.2 409.6 819.2 341.33"
        fill={LOGO_COLORS.yellow}
        initial={{ opacity: 0, y: -60 }}
        animate={{ opacity: 1, y: 0 }}
        transition={dropTransition(0.6)}
      />

      {/* Green layer - TOP, drops LAST (cool, private) */}
      <motion.polygon
        points="512 426.67 204.8 273.06 204.8 204.8 512 51.2 819.2 204.8 819.2 273.06 512 426.67"
        fill={LOGO_COLORS.green}
        initial={{ opacity: 0, y: -60 }}
        animate={{ opacity: 1, y: 0 }}
        transition={dropTransition(0.8)}
      />
    </motion.svg>
  );
}
