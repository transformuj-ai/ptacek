import type { MascotProps } from "../manifest";
import "./pixel.css";

// 12 Pixel-art pták — flappy sinusoida se skoky, steps() křídla.
// Master animace = k12fly na .a12.

function Pixel({ text, onDone }: MascotProps) {
  return (
    <div className="scene s12">
      <div
        className="act a12"
        onAnimationEnd={(e) => {
          if (e.animationName === "k12fly") onDone();
        }}
      >
        <div className="a12b">
          <div className="bub sq b12">{text}</div>
          <svg className="px" viewBox="0 0 44 32" aria-hidden="true">
            <g fill="#F97316">
              <rect x="8" y="4" width="20" height="4" />
              <rect x="4" y="8" width="28" height="4" />
              <rect x="4" y="12" width="32" height="4" />
              <rect x="4" y="16" width="32" height="4" />
              <rect x="8" y="20" width="24" height="4" />
              <rect x="12" y="24" width="16" height="4" />
            </g>
            <g fill="#FAFAFA">
              <rect x="12" y="20" width="12" height="4" />
              <rect x="16" y="24" width="8" height="4" />
              <rect x="24" y="8" width="4" height="4" />
            </g>
            <rect x="26" y="10" width="3" height="3" fill="#0A0A0B" />
            <g fill="#C2410C">
              <rect x="32" y="12" width="8" height="4" />
              <rect x="36" y="16" width="4" height="3" />
            </g>
            <rect x="0" y="12" width="6" height="4" fill="#C2410C" />
            <g className="pxwing" fill="#FAFAFA">
              <rect x="10" y="12" width="14" height="4" />
              <rect x="12" y="16" width="10" height="4" />
            </g>
          </svg>
        </div>
      </div>
    </div>
  );
}

export default Pixel;
