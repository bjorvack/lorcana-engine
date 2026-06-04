import { describe, expect, it } from 'vitest';
import { facedownCards, toDisplayCard } from './cardModel';
import type { CardDef, CardView } from './engine';

const def: CardDef = {
  defId: 1,
  name: 'Mickey Mouse - True Friend',
  image: 'https://example.test/mickey.avif',
  cardType: 'Character',
  cost: 3,
  inkwell: true,
  strength: 2,
  willpower: 3,
  lore: 2,
  moveCost: undefined,
  classifications: ['Storyborn', 'Hero'],
  text: undefined,
};

const cards = new Map<number, CardDef>([[def.defId, def]]);

function instance(overrides: Partial<CardView> = {}): CardView {
  return {
    instanceId: 10,
    defId: 1,
    ready: true,
    drying: false,
    facedown: false,
    damage: 0,
    strength: undefined,
    willpower: undefined,
    lore: undefined,
    atLocation: undefined,
    under: [],
    ...overrides,
  };
}

describe('toDisplayCard', () => {
  it('merges printed data and resolves the name/art', () => {
    const card = toDisplayCard(instance(), cards);
    expect(card.name).toBe('Mickey Mouse - True Friend');
    expect(card.image).toBe(def.image);
    expect(card.cost).toBe(3);
    expect(card.type).toBe('Character');
  });

  it('prefers live stats over printed when present', () => {
    const card = toDisplayCard(instance({ strength: 5 }), cards);
    expect(card.strength).toBe(5);
    expect(card.willpower).toBe(3); // falls back to printed
  });

  it('treats exerted as the inverse of ready', () => {
    expect(toDisplayCard(instance({ ready: false }), cards).exerted).toBe(true);
    expect(toDisplayCard(instance({ ready: true }), cards).exerted).toBe(false);
  });

  it('computes remaining willpower and lethal damage', () => {
    const hurt = toDisplayCard(instance({ damage: 2 }), cards);
    expect(hurt.willpowerRemaining).toBe(1);
    expect(hurt.lethal).toBe(false);

    const dead = toDisplayCard(instance({ damage: 3 }), cards);
    expect(dead.willpowerRemaining).toBe(0);
    expect(dead.lethal).toBe(true);
  });

  it('falls back gracefully for unknown definitions', () => {
    const card = toDisplayCard(instance({ defId: 999 }), new Map());
    expect(card.name).toBe('#999');
    expect(card.willpower).toBeUndefined();
    expect(card.lethal).toBe(false);
  });
});

describe('facedownCards', () => {
  it('produces the requested count with unique negative ids', () => {
    const placeholders = facedownCards(3);
    expect(placeholders).toHaveLength(3);
    expect(placeholders.every((c) => c.facedown)).toBe(true);
    const ids = new Set(placeholders.map((c) => c.instanceId));
    expect(ids.size).toBe(3);
    expect([...ids].every((id) => id < 0)).toBe(true);
  });
});
