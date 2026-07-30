import type { MascotProps } from "../manifest";
import "./cat.css";

// 04 Kocour po spodní hraně — dojde do třetiny, zastaví se, otočí hlavu
// na uživatele, ukáže ceduli a jde dál. Master animace = k4walk na .a4.

function Cat({ text, onDone }: MascotProps) {
  return (
    <div className="scene s4">
      <div
        className="act a4"
        onAnimationEnd={(e) => {
          if (e.animationName === "k4walk") onDone();
        }}
      >
        <div className="bub sq sign4">{text}</div>
        <svg className="cat" viewBox="0 0 96 62" aria-hidden="true">
          <path
            className="tail"
            d="M8,34 C0,32 2,18 12,20"
            fill="none"
            stroke="#F97316"
            strokeWidth="4.5"
            strokeLinecap="round"
          />
          <rect className="legB" x="20" y="38" width="5.5" height="18" rx="2.6" fill="#C2410C" />
          <rect className="legA" x="62" y="38" width="5.5" height="18" rx="2.6" fill="#C2410C" />
          <rect className="legA" x="30" y="38" width="5.5" height="18" rx="2.6" fill="#F97316" />
          <rect className="legB" x="72" y="38" width="5.5" height="18" rx="2.6" fill="#F97316" />
          <rect x="12" y="20" width="70" height="24" rx="12" fill="#F97316" />
          <g className="headg">
            <circle cx="80" cy="20" r="13" fill="#F97316" />
            <path d="M69,12 L70,2 L78,9 Z" fill="#F97316" />
            <path d="M91,12 L92,2 L84,9 Z" fill="#F97316" />
            <g className="fside">
              <circle cx="87" cy="19" r="2.1" fill="#0A0A0B" />
              <path
                d="M92,24 q-4,3 -8,1"
                fill="none"
                stroke="#0A0A0B"
                strokeWidth="1.4"
                strokeLinecap="round"
                opacity=".7"
              />
            </g>
            <g className="ffront">
              <circle cx="75" cy="19" r="2.1" fill="#0A0A0B" />
              <circle cx="85" cy="19" r="2.1" fill="#0A0A0B" />
              <path
                d="M76,25 q4,3 8,0"
                fill="none"
                stroke="#0A0A0B"
                strokeWidth="1.4"
                strokeLinecap="round"
                opacity=".7"
              />
            </g>
            <path
              d="M76,22 L80,25 L84,22"
              fill="none"
              stroke="#FFFFFF"
              strokeWidth="1.4"
              strokeLinecap="round"
            />
          </g>
          <path
            d="M18,30 h56"
            stroke="#FFFFFF"
            strokeWidth="2"
            opacity=".28"
            strokeLinecap="round"
          />
        </svg>
      </div>
    </div>
  );
}

export default Cat;
