import type { MascotProps } from "../manifest";
import "./robot.css";

// 08 Robot poslíček — přijede po spodku, zamává anténkou, zvedne ceduli
// a odjede. Master animace = k8ride na .a8.

function Robot({ text, onDone }: MascotProps) {
  return (
    <div className="scene s8">
      <div
        className="act a8"
        onAnimationEnd={(e) => {
          if (e.animationName === "k8ride") onDone();
        }}
      >
        <div className="bub sq sign8">{text}</div>
        <svg className="robot" viewBox="0 0 92 84" aria-hidden="true">
          <g className="ant">
            <path d="M46,20 L46,6" stroke="#D4D4D8" strokeWidth="2.6" strokeLinecap="round" />
            <circle cx="46" cy="4" r="4.2" fill="#F97316" />
          </g>
          <rect x="26" y="18" width="40" height="26" rx="6" fill="#FAFAFA" />
          <rect x="33" y="26" width="26" height="10" rx="3" fill="#0A0A0B" />
          <circle cx="41" cy="31" r="2.2" fill="#F97316" />
          <circle cx="51" cy="31" r="2.2" fill="#F97316" />
          <rect x="20" y="46" width="52" height="22" rx="5" fill="#F97316" />
          <rect x="28" y="52" width="16" height="4" rx="2" fill="#FFFFFF" opacity=".65" />
          <rect x="28" y="59" width="26" height="3" rx="1.5" fill="#FFFFFF" opacity=".35" />
          <rect x="72" y="48" width="6" height="18" rx="3" fill="#D4D4D8" />
          <circle className="wheel" cx="32" cy="73" r="9" fill="#232327" stroke="#A1A1AA" strokeWidth="2.4" />
          <circle className="wheel" cx="60" cy="73" r="9" fill="#232327" stroke="#A1A1AA" strokeWidth="2.4" />
          <path d="M32,64 v18 M23,73 h18" stroke="#A1A1AA" strokeWidth="1.4" opacity=".5" />
          <path d="M60,64 v18 M51,73 h18" stroke="#A1A1AA" strokeWidth="1.4" opacity=".5" />
        </svg>
      </div>
    </div>
  );
}

export default Robot;
