import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import type { DisplayCard } from '../cardModel';
import Card from './Card.svelte';

function display(overrides: Partial<DisplayCard> = {}): DisplayCard {
  return {
    instanceId: 1,
    defId: 1,
    name: 'Stitch - New Dog',
    image: undefined,
    cost: 1,
    type: 'Character',
    exerted: false,
    drying: false,
    facedown: false,
    damage: 0,
    strength: 2,
    willpower: 2,
    lore: 1,
    willpowerRemaining: 2,
    lethal: false,
    isLocation: false,
    underCount: 0,
    ...overrides,
  };
}

describe('Card', () => {
  it('renders a faceup card with its stats and name', () => {
    const { getByText, getByTitle } = render(Card, { card: display() });
    expect(getByTitle('Stitch - New Dog')).toBeInTheDocument();
    expect(getByText('1', { selector: '.cost' })).toBeInTheDocument();
    expect(getByText('2/2')).toBeInTheDocument(); // willpower remaining/printed
  });

  it('hides details for a facedown card', () => {
    const { queryByText, getByAltText } = render(Card, {
      card: display({ facedown: true }),
    });
    expect(getByAltText('Card back')).toBeInTheDocument();
    expect(queryByText('2/2')).not.toBeInTheDocument();
  });

  it('shows a damage chip only when damaged', () => {
    const { getByText } = render(Card, {
      card: display({ damage: 1, willpowerRemaining: 1 }),
    });
    expect(getByText('1', { selector: '.damage' })).toBeInTheDocument();
  });
});
