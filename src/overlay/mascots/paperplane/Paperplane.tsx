import type { MascotProps } from "../manifest";
import "./paperplane.css";

// 02 Papírová vlaštovka — origami z popsaného papíru, klouže po diagonále.
// Master animace = k2glide na .a2.

function Paperplane({ text, onDone }: MascotProps) {
  return (
    <div className="scene s2">
      <div
        className="act a2"
        onAnimationEnd={(e) => {
          if (e.animationName === "k2glide") onDone();
        }}
      >
        <div className="a2b">
          <div className="bub b2">{text}</div>
          <svg className="plane2" viewBox="0 0 84 46" aria-hidden="true">
            <path
              d="M2,7 L82,22 L2,39 L17,23 Z"
              fill="#FFFFFF"
              stroke="#F97316"
              strokeWidth="1.6"
              strokeLinejoin="round"
            />
            <path d="M17,23 L82,22 L2,39 Z" fill="#D4D4D8" opacity=".75" />
            <g stroke="#71717A" strokeWidth="1.3" strokeLinecap="round">
              <path d="M13,16 h16" />
              <path d="M17,20 h20" />
              <path d="M24,12 h10" />
            </g>
            <path
              d="M2,7 L17,23 L2,39"
              fill="none"
              stroke="#C2410C"
              strokeWidth="1.2"
            />
          </svg>
        </div>
      </div>
    </div>
  );
}

export default Paperplane;
