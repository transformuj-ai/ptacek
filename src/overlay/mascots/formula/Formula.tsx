import type { MascotProps } from "../manifest";
import "./formula.css";

// 09 Formule — prosviští, text zůstane ve stopě a vybledne.
// Master animace = k9dash na .a9 (končí na 100 % cyklu, tedy až po
// zmizení textu — okno se nezavře dřív, než dohraje celá scéna).

function Formula({ text, onDone }: MascotProps) {
  return (
    <div className="scene s9">
      <div className="trail" />
      <div className="t9">{text}</div>
      <div
        className="act a9"
        onAnimationEnd={(e) => {
          if (e.animationName === "k9dash") onDone();
        }}
      >
        <svg className="f1" viewBox="0 0 140 44" aria-hidden="true">
          <path
            d="M4,30 L28,30 L40,20 L86,18 L96,26 L134,28 L136,34 L4,34 Z"
            fill="#F97316"
          />
          <path d="M84,18 L92,6 L104,7 L100,18 Z" fill="#C2410C" />
          <rect x="52" y="12" width="18" height="8" rx="4" fill="#FAFAFA" />
          <rect x="0" y="24" width="18" height="4" rx="2" fill="#FAFAFA" />
          <rect x="120" y="14" width="20" height="4" rx="2" fill="#FAFAFA" />
          <circle
            className="fw"
            cx="34"
            cy="34"
            r="9"
            fill="#18181B"
            stroke="#A1A1AA"
            strokeWidth="2"
          />
          <circle
            className="fw"
            cx="108"
            cy="34"
            r="9"
            fill="#18181B"
            stroke="#A1A1AA"
            strokeWidth="2"
          />
          <path d="M34,26 v16" stroke="#A1A1AA" strokeWidth="1.6" opacity=".55" />
          <path d="M108,26 v16" stroke="#A1A1AA" strokeWidth="1.6" opacity=".55" />
        </svg>
      </div>
    </div>
  );
}

export default Formula;
