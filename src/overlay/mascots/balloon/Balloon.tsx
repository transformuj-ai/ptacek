import type { MascotProps } from "../manifest";
import "./balloon.css";

// 10 Balón s košem — pomalu propluje horní třetinou, text přímo na
// balónu. Master animace = k10drift na .a10.

function Balloon({ text, onDone }: MascotProps) {
  return (
    <div className="scene s10">
      <div
        className="act a10"
        onAnimationEnd={(e) => {
          if (e.animationName === "k10drift") onDone();
        }}
      >
        <div className="a10b">
          <svg className="balloon" viewBox="0 0 90 118" aria-hidden="true">
            <path
              d="M45,2 C69,2 86,22 86,44 C86,63 70,76 56,84 L34,84 C20,76 4,63 4,44 C4,22 21,2 45,2Z"
              fill="#F97316"
            />
            <g stroke="#FFFFFF" strokeWidth="1.6" fill="none" opacity=".55">
              <path d="M22,8 C15,24 14,58 27,80" />
              <path d="M68,8 C75,24 76,58 63,80" />
            </g>
            <path
              d="M45,2 C69,2 86,22 86,44 C86,52 83,59 78,65 C80,44 70,16 45,10 C20,16 10,44 12,65 C7,59 4,52 4,44 C4,22 21,2 45,2Z"
              fill="#FFFFFF"
              opacity=".18"
            />
            <path d="M34,84 L38,99 M56,84 L52,99" stroke="#D4D4D8" strokeWidth="1.6" />
            <rect x="35" y="97" width="20" height="14" rx="3" fill="#71717A" />
            <rect x="35" y="97" width="20" height="4" rx="2" fill="#A1A1AA" />
          </svg>
          <div className="t10">{text}</div>
        </div>
      </div>
    </div>
  );
}

export default Balloon;
