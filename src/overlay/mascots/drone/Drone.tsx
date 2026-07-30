import type { MascotProps } from "../manifest";
import "./drone.css";

// 07 Dron s balíčkem — přiletí, upustí kartičku s textem, ta se snese
// kývavým pádem a vybledne. Master animace = k7fly na .a7.

function Drone({ text, onDone }: MascotProps) {
  return (
    <div className="scene s7">
      <div className="pkgw">
        <div className="pkg">{text}</div>
      </div>
      <div
        className="act a7"
        onAnimationEnd={(e) => {
          if (e.animationName === "k7fly") onDone();
        }}
      >
        <div className="a7b">
          <svg className="drone" viewBox="0 0 120 60" aria-hidden="true">
            <rect x="42" y="24" width="36" height="18" rx="6" fill="#F97316" />
            <rect x="52" y="29" width="16" height="8" rx="3" fill="#0A0A0B" opacity=".6" />
            <path
              d="M42,28 L18,18 M78,28 L102,18"
              stroke="#D4D4D8"
              strokeWidth="3"
              strokeLinecap="round"
            />
            <rect className="rot" x="2" y="14" width="32" height="4" rx="2" fill="#FAFAFA" />
            <rect className="rot" x="86" y="14" width="32" height="4" rx="2" fill="#FAFAFA" />
            <circle cx="18" cy="16" r="2.6" fill="#F97316" />
            <circle cx="102" cy="16" r="2.6" fill="#F97316" />
            <path
              d="M48,42 v6 M72,42 v6"
              stroke="#A1A1AA"
              strokeWidth="2.4"
              strokeLinecap="round"
            />
          </svg>
        </div>
      </div>
    </div>
  );
}

export default Drone;
