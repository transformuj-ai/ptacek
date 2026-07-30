import type { ComponentType } from "react";
import Bird from "./bird/Bird";
import Paperplane from "./paperplane/Paperplane";
import Plane from "./plane/Plane";
import Cat from "./cat/Cat";
import Drone from "./drone/Drone";
import Robot from "./robot/Robot";
import Formula from "./formula/Formula";
import Balloon from "./balloon/Balloon";
import Flock from "./flock/Flock";
import Pixel from "./pixel/Pixel";
import "./shared.css";

// Registr maskotů. Každý maskot je čistá React komponenta s CSS animací
// přenesenou 1:1 ze showcase (vyber-variant.html). Kontrakt:
// - `text` se renderuje výhradně jako text node (žádné HTML),
// - konec master animace hlásí `onDone()` → Rust zavře overlay okno,
// - jednotky cqw se vážou na .overlay-root (container-type: inline-size).

export interface MascotProps {
  text: string;
  onDone: () => void;
}

export interface MascotDef {
  id: string;
  nazev: string;
  nameEn: string;
  /** délka celé scény v ms — informativní (master animace řídí konec) */
  duration: number;
  component: ComponentType<MascotProps>;
  /** výchozí text pro demo režim */
  demoText: string;
  descCs: string;
  descEn: string;
}

export const MASCOTS: MascotDef[] = [
  { id: "bird", nazev: "Oranžový ptáček t!", nameEn: "Orange bird t!", duration: 8000, component: Bird, demoText: "Schůzka za 5 minut",
    descCs: "Klasika. Letí vlnovkou zleva doprava, mává křídly a táhne bublinu s názvem schůzky. Výchozí maskot.",
    descEn: "The classic. Flies left to right in a wave, flapping wings, pulling a bubble with your meeting title. The default mascot." },
  { id: "paperplane", nazev: "Papírová vlaštovka", nameEn: "Paper plane", duration: 8000, component: Paperplane, demoText: "Standup teď",
    descCs: "Origami vlaštovka tiše klouže po diagonále zdola nahoru. Nejelegantnější z party.",
    descEn: "An origami plane glides quietly along a diagonal from bottom to top. The most elegant of the bunch." },
  { id: "plane", nazev: "Letadélko s transparentem", nameEn: "Plane with a banner", duration: 9000, component: Plane, demoText: "Porada s vedením · za 5 min",
    descCs: "Táhne za sebou vlající plachtu s celým textem pozvánky. Nejčitelnější pro dlouhé názvy schůzek.",
    descEn: "Pulls a waving banner with the full invite text. Most readable for long meeting titles." },
  { id: "cat", nazev: "Kocour", nameEn: "Cat", duration: 10000, component: Cat, demoText: "Meeting!",
    descCs: "Projde po spodní hraně obrazovky, v půlce se zastaví, podívá se na tebe, zvedne ceduli a jde dál.",
    descEn: "Walks along the bottom edge, stops halfway, looks at you, raises a sign, and walks on." },
  { id: "drone", nazev: "Dron s balíčkem", nameEn: "Delivery drone", duration: 9000, component: Drone, demoText: "14:00 · Standup",
    descCs: "Přiletí, zavisí ve vzduchu a upustí kartičku se schůzkou, která se snese jako peříčko.",
    descEn: "Flies in, hovers, and drops a meeting card that floats down like a feather." },
  { id: "robot", nazev: "Robot poslíček", nameEn: "Courier robot", duration: 10000, component: Robot, demoText: "Schůzka za 2 min",
    descCs: "Přijede po spodku obrazovky, zamává anténkou, zvedne ceduli se schůzkou a odjede.",
    descEn: "Rolls along the bottom, wiggles its antenna, raises a meeting sign, and rolls away." },
  { id: "formula", nazev: "Formule", nameEn: "Formula car", duration: 6000, component: Formula, demoText: "Jdeš pozdě!",
    descCs: "Prosviští přes obrazovku za zlomek vteřiny a v kouřové stopě nechá vzkaz. Pro chvíle, kdy už fakt jdeš pozdě.",
    descEn: "Zooms across in a split second, leaving a message in its smoke trail. For when you're really late." },
  { id: "balloon", nazev: "Balón s košem", nameEn: "Hot-air balloon", duration: 14000, component: Balloon, demoText: "Za 10 minut call",
    descCs: "Pomalu propluje horní třetinou obrazovky s textem na balónu. Nejklidnější režim, skoro neruší.",
    descEn: "Drifts slowly across the top third with the text on the balloon. The calmest, least intrusive mode." },
  { id: "flock", nazev: "Hejno ptáků", nameEn: "Bird flock", duration: 7000, component: Flock, demoText: "TEĎ",
    descCs: "21 ptáčků vletí na obrazovku, na okamžik zformují slovo TEĎ a rozletí se pryč. Efektní.",
    descEn: "21 birds fly in, briefly form the word NOW, and scatter. Spectacular." },
  { id: "pixel", nazev: "Pixel-art pták", nameEn: "Pixel-art bird", duration: 8000, component: Pixel, demoText: "TEĎ!",
    descCs: "8-bitový pták letí flappy skoky přes obrazovku. Retro režim pro pamětníky.",
    descEn: "An 8-bit bird flappy-hops across the screen. Retro mode for the nostalgic." },
];

export function getMascot(id: string | null): MascotDef {
  const found = MASCOTS.find((m) => m.id === id);
  if (found) return found;
  // "random" i neznámé id → náhodný výběr z dostupných
  return MASCOTS[Math.floor(Math.random() * MASCOTS.length)];
}
