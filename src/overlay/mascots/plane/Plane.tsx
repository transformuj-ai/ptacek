import type { MascotProps } from "../manifest";
import "./plane.css";

// 03 Letadélko s transparentem — nejčitelnější režim pro delší texty
// (aerobanner). Master animace = k3fly na .a3.
// Pozn.: banner je PŘED letadlem v DOM (flex řadí: banner—lanko—letadlo),
// letadlo letí doprava a táhne plachtu za sebou.

function Plane({ text, onDone }: MascotProps) {
  return (
    <div className="scene s3">
      <div
        className="act a3"
        onAnimationEnd={(e) => {
          if (e.animationName === "k3fly") onDone();
        }}
      >
        <div className="banner">{text}</div>
        <span className="rope" />
        <svg className="plane3" viewBox="0 0 84 40" aria-hidden="true">
          <path
            d="M6,22 C14,14 34,11 54,13 C66,14 74,17 78,20 C72,25 58,29 40,29 C24,29 12,27 6,22Z"
            fill="#F97316"
          />
          <path d="M28,14 L36,3 L45,4 L40,15 Z" fill="#FFFFFF" />
          <path d="M8,20 L2,10 L9,11 L14,19 Z" fill="#FFFFFF" />
          <circle cx="62" cy="19" r="2.6" fill="#0A0A0B" opacity=".55" />
          <rect
            className="prop"
            x="76"
            y="11"
            width="2.8"
            height="19"
            rx="1.4"
            fill="#D4D4D8"
          />
        </svg>
      </div>
    </div>
  );
}

export default Plane;
