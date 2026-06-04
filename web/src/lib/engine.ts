import init, { WasmGame } from './wasm/lorcana_wasm';
import type { CardDef, GameView } from './wasm/lorcana_wasm';

export type { CardDef, CardView, GameView, InkwellView, PlayerView } from './wasm/lorcana_wasm';

let ready: Promise<unknown> | null = null;

/** Initialise the WASM module once (idempotent). */
async function initEngine(): Promise<void> {
  ready ??= init();
  await ready;
}

/**
 * A typed, UI-facing facade over the WASM {@link WasmGame}. Holds the immutable
 * card database (printed data, keyed by `defId`) and exposes the current board
 * {@link GameView} plus a `step()` driver for the read-only demo.
 */
export class Engine {
  readonly #game: WasmGame;
  /** Printed card data, keyed by `defId`. */
  readonly cards: ReadonlyMap<number, CardDef>;

  private constructor(game: WasmGame) {
    this.#game = game;
    this.cards = new Map(game.cardDb().map((card) => [card.defId, card]));
  }

  /** Create and start a game from the bundled decklists for the given seed. */
  static async create(seed: bigint): Promise<Engine> {
    await initEngine();
    return new Engine(new WasmGame(seed));
  }

  /** The current board view. */
  view(): GameView {
    return this.#game.view();
  }

  /** Advance the demo by one pseudo-random legal action; `false` when stuck/over. */
  step(): boolean {
    return this.#game.stepRandom();
  }

  /** Whether the game has finished. */
  get finished(): boolean {
    return this.#game.isFinished();
  }
}
