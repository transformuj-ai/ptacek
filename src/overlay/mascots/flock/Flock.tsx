import { useRef } from "react";
import type { MascotProps } from "../manifest";
import "./flock.css";

// 11 Hejno ptáků — 21 ptáčků zformuje slovo „TEĎ" a rozletí se.
// Port generátoru ze showcase do React .map() — žádný innerHTML.
// Slovo je fixní (text schůzky se u hejna ukáže až v hover kartě, PR7).
// Všech 21 animací k11 končí naráz; onDone hlásíme jen jednou.

// Souřadnice formace: původně cqw vůči 16:9 stage → tx ve vw, ty ve vh
// (převod: ty_vh = cqw / 56.25 * 100), mřížka 3 řádky.
const PTS: Array<[number, number]> = [
  // T
  [13, 28], [20, 28], [27, 28], [20, 48], [20, 68],
  // E
  [40, 28], [47, 28], [54, 28], [40, 48], [47, 48], [40, 68], [47, 68], [54, 68],
  // D
  [67, 28], [74, 28], [81, 37], [81, 57], [74, 68], [67, 68], [67, 48],
  // háček nad Ď
  [78, 12],
];

function Flock({ onDone }: MascotProps) {
  const done = useRef(false);

  return (
    <div className="scene s11">
      {PTS.map(([tx, ty], i) => (
        <span
          key={i}
          className="bd"
          style={{ "--tx": `${tx}vw`, "--ty": `${ty}vh` } as React.CSSProperties}
          onAnimationEnd={(e) => {
            if (e.animationName === "k11" && !done.current) {
              done.current = true;
              onDone();
            }
          }}
        >
          <svg viewBox="0 0 22 14" aria-hidden="true">
            <path
              d="M1,12 C4,3 8,2 11,8 C14,2 18,3 21,12"
              fill="none"
              stroke="#F97316"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </span>
      ))}
    </div>
  );
}

export default Flock;
