import type { CardDef, CardView } from './engine';

/** A fully-resolved card ready to render: dynamic instance state merged with its
 *  printed definition, with a few derived display fields. */
export interface DisplayCard {
  instanceId: number;
  defId: number;
  name: string;
  image: string | undefined;
  cost: number;
  type: string;
  /** Turned sideways (not ready). */
  exerted: boolean;
  /** Summoning sick ("wet ink"). */
  drying: boolean;
  /** Shown as a card back. */
  facedown: boolean;
  damage: number;
  strength: number | undefined;
  willpower: number | undefined;
  lore: number | undefined;
  /** Willpower left before banishment, or `undefined` when the card has none. */
  willpowerRemaining: number | undefined;
  /** Damage has reached willpower (about to be / is banished). */
  lethal: boolean;
  isLocation: boolean;
  /** Number of cards stacked beneath (Shift/Boost). */
  underCount: number;
}

/**
 * Merge a {@link CardView} (dynamic state) with its {@link CardDef} (printed
 * data) into a renderable {@link DisplayCard}. Live stats win over printed ones.
 */
export function toDisplayCard(card: CardView, cards: ReadonlyMap<number, CardDef>): DisplayCard {
  const def = cards.get(card.defId);
  const strength = card.strength ?? def?.strength;
  const willpower = card.willpower ?? def?.willpower;
  const lore = card.lore ?? def?.lore;
  const willpowerRemaining = willpower === undefined ? undefined : Math.max(0, willpower - card.damage);

  return {
    instanceId: card.instanceId,
    defId: card.defId,
    name: def?.name ?? `#${card.defId}`,
    image: def?.image,
    cost: def?.cost ?? 0,
    type: def?.cardType ?? 'Character',
    exerted: !card.ready,
    drying: card.drying,
    facedown: card.facedown,
    damage: card.damage,
    strength,
    willpower,
    lore,
    willpowerRemaining,
    lethal: willpower !== undefined && card.damage >= willpower,
    isLocation: (def?.cardType ?? '') === 'Location',
    underCount: card.under.length,
  };
}

/** Build `count` facedown placeholder cards (for a hidden hand). The synthetic
 *  negative instance ids never collide with real ones, keeping `{#each}` keys
 *  stable. */
export function facedownCards(count: number, offset = 0): DisplayCard[] {
  return Array.from({ length: count }, (_unused, i) => ({
    instanceId: -(offset + i + 1),
    defId: -1,
    name: 'Hidden',
    image: undefined,
    cost: 0,
    type: 'Character',
    exerted: false,
    drying: false,
    facedown: true,
    damage: 0,
    strength: undefined,
    willpower: undefined,
    lore: undefined,
    willpowerRemaining: undefined,
    lethal: false,
    isLocation: false,
    underCount: 0,
  }));
}
