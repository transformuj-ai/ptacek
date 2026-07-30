import type { MascotProps } from "../manifest";
import "./bird.css";

// 01 Oranžový ptáček t! — SVG i keyframes 1:1 ze showcase.
// Master animace = k1fly na .a1; její konec hlásíme nahoru (onDone).
// Text jde výhradně jako React text node — žádné HTML z dat.

function Bird({ text, onDone }: MascotProps) {
  return (
    <div className="scene s1">
      <div
        className="act a1"
        onAnimationEnd={(e) => {
          if (e.animationName === "k1fly") onDone();
        }}
      >
        <div className="a1b">
          <div className="bub b1">{text}</div>
          <svg className="bird1" viewBox="0 0 76 44" aria-hidden="true">
            <path
              d="M14,22 C14,12 24,5 38,6 C52,7 63,13 70,21 C60,30 46,36 32,34 C20,32 14,29 14,22Z"
              fill="#F97316"
            />
            <path
              d="M20,26 C28,32 44,34 58,29 C48,34 34,36 24,33Z"
              fill="#FFFFFF"
              opacity=".55"
            />
            <path d="M14,22 L1,10 L6,22 L1,34 Z" fill="#C2410C" />
            <path
              className="wing"
              d="M26,17 C33,7 48,7 54,17 C46,25 33,25 26,17Z"
              fill="#FFFFFF"
            />
            <path d="M68,18 L76,21 L68,25 Z" fill="#F5F5F5" />
            <circle cx="58" cy="16" r="2.1" fill="#0A0A0B" />
          </svg>
        </div>
      </div>
    </div>
  );
}

export default Bird;
