// Lightweight Shogi board renderer for documentation and demos.
// Standalone TypeScript module with no external runtime dependencies.

// Local declaration for LiveBoardAdapter to avoid external imports.
// This mirrors the minimal surface used by the renderer.
export interface LiveBoardAdapter {
  mount(target: Element): void;
  resize(): void;
  setPositionFromSFEN(sfen: string): void;
  setMoves(moves: readonly string[]): void;
  goTo(ply: number): void;
  flip(enabled: boolean): void;
  highlightSquares(indices: Iterable<number>): void;
  setTheme(theme: Partial<BoardTheme>): void;
  setOptions(options: Partial<{ showHands: boolean }>): void;
  setArrows(arrows: ArrowAnnotation[]): void;
  setCircles(circles: CircleAnnotation[]): void;
  clearAnnotations(): void;
  animate(frames: AnimationFrame[], options?: AnimationOptions): void;
  pauseAnimation(): void;
  resumeAnimation(): void;
  stopAnimation(): void;
  goToFrame(index: number): void;
  destroy(): void;
  dispose(): void;
}

enum PieceType {
  EMPTY = 0,
  PAWN = 1,
  LANCE = 2,
  KNIGHT = 3,
  SILVER = 4,
  GOLD = 5,
  BISHOP = 6,
  ROOK = 7,
  KING = 8,
  PRO_PAWN = 9,
  PRO_LANCE = 10,
  PRO_KNIGHT = 11,
  PRO_SILVER = 12,
  HORSE = 13,
  DRAGON = 14,
}

enum Color {
  BLACK = 0,
  WHITE = 1,
}

interface BoardPiece {
  type: PieceType;
  color: Color;
}

interface ParsedMove {
  from: { file: number; rank: number } | null;
  to: { file: number; rank: number };
  piece: PieceType;
  promote: boolean;
}

interface BoardTheme {
  boardColor: string;
  gridColor: string;
  blackPieceColor: string;
  whitePieceColor: string;
  lastMoveColor: string;
  highlightColor: string;
  coordinateColor: string;
}

const DEFAULT_THEME: BoardTheme = {
  boardColor: '#f0d9b5',
  gridColor: '#8b7355',
  blackPieceColor: '#000',
  whitePieceColor: '#000',
  lastMoveColor: 'rgba(50, 200, 50, 0.3)',
  highlightColor: 'rgba(255, 255, 0, 0.3)',
  coordinateColor: '#666',
};

// ---------------------------------------------------------------------------
// Annotation & Animation types
// ---------------------------------------------------------------------------

/**
 * Square specification for annotations.
 * - Board square: USI-style '7g' (file 1-9, rank a-i where a=1..i=9)
 * - Hand piece: 'S:P' (sente pawn), 'G:R' (gote rook)
 *   Color prefix S(ente)/G(ote), colon, piece letter P/L/N/S/G/B/R.
 */
type SquareSpec = string;

interface ResolvedBoardSquare {
  kind: 'board';
  file: number; // 1-9
  rank: number; // 1-9
}

interface ResolvedHandSquare {
  kind: 'hand';
  color: Color;
  pieceType: PieceType;
}

type ResolvedSquare = ResolvedBoardSquare | ResolvedHandSquare;

const HAND_PIECE_MAP: Record<string, PieceType> = {
  P: PieceType.PAWN,
  L: PieceType.LANCE,
  N: PieceType.KNIGHT,
  S: PieceType.SILVER,
  G: PieceType.GOLD,
  B: PieceType.BISHOP,
  R: PieceType.ROOK,
};

function parseSquareSpec(spec: string): ResolvedSquare | null {
  if (!spec) return null;
  // Board square: '7g'
  if (spec.length === 2) {
    const file = parseInt(spec[0], 10);
    const rankChar = spec[1].toLowerCase();
    if (file >= 1 && file <= 9 && rankChar >= 'a' && rankChar <= 'i') {
      const rank = rankChar.charCodeAt(0) - 96; // 'a'=1 .. 'i'=9
      return { kind: 'board', file, rank };
    }
  }
  // Hand square: 'S:P' or 'G:R'
  if (spec.length >= 3 && spec[1] === ':') {
    const colorChar = spec[0].toUpperCase();
    const pieceChar = spec[2].toUpperCase();
    const color = colorChar === 'S' ? Color.BLACK : colorChar === 'G' ? Color.WHITE : null;
    if (color === null) return null;
    const pt = HAND_PIECE_MAP[pieceChar];
    if (pt === undefined) return null;
    return { kind: 'hand', color, pieceType: pt };
  }
  return null;
}

export interface ArrowAnnotation {
  from: SquareSpec;
  to: SquareSpec;
  color?: string;  // default: 'rgba(0,120,215,0.6)'
  width?: number;  // line-width multiplier, default: 1.0
}

export interface CircleAnnotation {
  square: SquareSpec;
  color?: string;  // default: 'rgba(0,120,215,0.6)'
  width?: number;  // stroke-width multiplier, default: 1.0
}

export interface AnimationFrame {
  sfen?: string;
  ply?: number;
  arrows?: ArrowAnnotation[];
  circles?: CircleAnnotation[];
  highlights?: number[];
  duration?: number; // ms, default: 1000
}

export interface AnimationOptions {
  loop?: boolean;          // default: false
  defaultDuration?: number; // default: 1000
  autoPlay?: boolean;       // default: true
}

interface BoardLayout {
  offsetX: number;
  offsetY: number;
  cellSize: number;
  boardSize: number;
  regionW: number;
  regionH: number;
  regionXSente: number;
  regionXGote: number;
  senteRegionY: number;
  goteRegionY: number;
  width: number;
  height: number;
}

const enum AnimationPlayState {
  STOPPED = 0,
  PLAYING = 1,
  PAUSED = 2,
}

const DEFAULT_ARROW_COLOR = 'rgba(0, 120, 215, 0.6)';
const DEFAULT_CIRCLE_COLOR = 'rgba(0, 120, 215, 0.6)';

class BoardState {
  board: (BoardPiece | null)[][];
  hands: Map<Color, Map<PieceType, number>>;
  currentTurn: Color;

  constructor() {
    this.board = Array.from({ length: 9 }, () => Array<BoardPiece | null>(9).fill(null));
    this.hands = new Map<Color, Map<PieceType, number>>([
      [Color.BLACK, new Map<PieceType, number>()],
      [Color.WHITE, new Map<PieceType, number>()],
    ]);
    this.currentTurn = Color.BLACK;
  }

  setPositionFromSFEN(sfen: unknown): void {
    const STARTPOS_SFEN = 'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1';

    const normalizeToken = (input: unknown): string => {
      const raw = String(input ?? '').trim();
      if (!raw) return 'startpos';
      let text = raw;
      if (text.startsWith('position ')) {
        text = text.slice('position '.length).trim();
      }
      if (text.startsWith('startpos')) {
        return 'startpos';
      }
      if (text.startsWith('sfen ')) {
        const tokens = text.split(/\s+/);
        if (tokens.length >= 5) {
          return tokens.slice(1, 5).join(' ');
        }
        return STARTPOS_SFEN;
      }
      const tokens = text.split(/\s+/);
      const movesIdx = tokens.indexOf('moves');
      if (movesIdx > -1) {
        text = tokens.slice(0, movesIdx).join(' ');
      }
      return text || 'startpos';
    };

    const applyToken = (token: string): boolean => {
      try {
        const normalized = token === 'startpos' || token === '' ? STARTPOS_SFEN : token;
        const parts = normalized.split(' ');
        if (parts.length < 3) {
          return false;
        }

        const blackHands = this.hands.get(Color.BLACK);
        const whiteHands = this.hands.get(Color.WHITE);
        if (!blackHands || !whiteHands) {
          return false;
        }

        this.board = Array.from({ length: 9 }, () => Array<BoardPiece | null>(9).fill(null));
        blackHands.clear();
        whiteHands.clear();

        const rankSpecs = parts[0]?.split('/') ?? [];
        if (rankSpecs.length !== 9) {
          return false;
        }

        for (let rank = 0; rank < rankSpecs.length; rank++) {
          const row = rankSpecs[rank] ?? '';
          let file = 0;
          let promoteNext = false;
          for (let idx = 0; idx < row.length; idx++) {
            const char = row[idx];
            if (char === '+') {
              if (promoteNext) {
                return false;
              }
              promoteNext = true;
              continue;
            }
            if (char >= '1' && char <= '9') {
              file += parseInt(char, 10);
              if (file > 9) {
                return false;
              }
              promoteNext = false;
              continue;
            }
            let piece = this.charToPiece(char);
            if (!piece) {
              return false;
            }
            if (promoteNext) {
              piece = { type: this.promoteType(piece.type), color: piece.color };
              promoteNext = false;
            }
            if (file >= 9) {
              return false;
            }
            this.board[rank][file] = piece;
            file += 1;
          }
          if (file !== 9) {
            return false;
          }
        }

        const turnPart = parts[1];
        if (turnPart !== 'b' && turnPart !== 'w') {
          return false;
        }
        this.currentTurn = turnPart === 'b' ? Color.BLACK : Color.WHITE;

        const handsStr = parts[2];
        if (handsStr && handsStr !== '-') {
          let count = 0;
          for (let i = 0; i < handsStr.length; i++) {
            const ch = handsStr[i];
            if (ch >= '0' && ch <= '9') {
              count = count * 10 + parseInt(ch, 10);
              continue;
            }
            const pieceType = this.handCharToPieceType(ch);
            if (pieceType == null) {
              return false;
            }
            const color = ch === ch.toUpperCase() ? Color.BLACK : Color.WHITE;
            const target = this.hands.get(color);
            if (!target) {
              return false;
            }
            target.set(pieceType, count || 1);
            count = 0;
          }
          if (count !== 0) {
            return false;
          }
        }
        return true;
      } catch (_error) {
        return false;
      }
    };

    const token = normalizeToken(sfen);
    if (applyToken(token)) {
      return;
    }

    throw new Error(`[arena-ui] Invalid SFEN token encountered: ${String(sfen)}`);
  }

  charToPiece(char: string): BoardPiece | null {
    const isBlack = char === char.toUpperCase();
    const color = isBlack ? Color.BLACK : Color.WHITE;
    const lowerChar = char.toLowerCase();
    const pieceMap: Record<string, PieceType> = {
      p: PieceType.PAWN,
      l: PieceType.LANCE,
      n: PieceType.KNIGHT,
      s: PieceType.SILVER,
      g: PieceType.GOLD,
      b: PieceType.BISHOP,
      r: PieceType.ROOK,
      k: PieceType.KING,
    };
    const type = pieceMap[lowerChar] ?? null;
    return type != null ? { type, color } : null;
  }

  promoteType(type: PieceType): PieceType {
    switch (type) {
      case PieceType.PAWN:
        return PieceType.PRO_PAWN;
      case PieceType.LANCE:
        return PieceType.PRO_LANCE;
      case PieceType.KNIGHT:
        return PieceType.PRO_KNIGHT;
      case PieceType.SILVER:
        return PieceType.PRO_SILVER;
      case PieceType.BISHOP:
        return PieceType.HORSE;
      case PieceType.ROOK:
        return PieceType.DRAGON;
      default:
        return type;
    }
  }

  demoteType(type: PieceType): PieceType {
    switch (type) {
      case PieceType.PRO_PAWN:
        return PieceType.PAWN;
      case PieceType.PRO_LANCE:
        return PieceType.LANCE;
      case PieceType.PRO_KNIGHT:
        return PieceType.KNIGHT;
      case PieceType.PRO_SILVER:
        return PieceType.SILVER;
      case PieceType.HORSE:
        return PieceType.BISHOP;
      case PieceType.DRAGON:
        return PieceType.ROOK;
      default:
        return type;
    }
  }

  handCharToPieceType(char: string): PieceType | null {
    const lowerChar = char.toLowerCase();
    const pieceMap: Record<string, PieceType> = {
      p: PieceType.PAWN,
      l: PieceType.LANCE,
      n: PieceType.KNIGHT,
      s: PieceType.SILVER,
      g: PieceType.GOLD,
      b: PieceType.BISHOP,
      r: PieceType.ROOK,
    };
    return Object.hasOwn(pieceMap, lowerChar) ? pieceMap[lowerChar] : null;
  }

  getPiece(file: number, rank: number): BoardPiece | null {
    return this.board[rank - 1]?.[9 - file] ?? null;
  }

  getHands(color: Color): Map<PieceType, number> {
    const existing = this.hands.get(color);
    if (existing) return existing;
    const created = new Map<PieceType, number>();
    this.hands.set(color, created);
    return created;
  }

  getTurn(): Color {
    return this.currentTurn;
  }
}

interface RendererOptions {
  showHands: boolean;
}

class ShogiBoardRenderer {
  private container: HTMLElement;
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private boardState: BoardState;
  private isFlipped: boolean;
  private highlightedSquares: Set<string>;
  private lastMove: ParsedMove | null;
  private moves: string[];
  private initialSfen: string;
  private pixelRatio: number;
  private logicalWidth: number;
  private logicalHeight: number;
  private theme: BoardTheme;
  private options: RendererOptions;
  private readonly resizeHandler: () => void;
  private layout: BoardLayout | null;
  private arrows: ArrowAnnotation[];
  private circles: CircleAnnotation[];
  private animFrames: AnimationFrame[];
  private animOptions: AnimationOptions;
  private animState: AnimationPlayState;
  private animCurrentFrame: number;
  private animTimerId: number;
  private animFrameStartTime: number;

  constructor(container: HTMLElement) {
    if (!(container instanceof HTMLElement)) {
      throw new Error('ShogiBoardRenderer requires an HTMLElement container');
    }
    this.container = container;
    const doc = container.ownerDocument ?? document;
    this.canvas = doc.createElement('canvas');
    const context = this.canvas.getContext('2d');
    if (!context) {
      throw new Error('Failed to get canvas context');
    }
    this.ctx = context;
    this.container.appendChild(this.canvas);

    this.boardState = new BoardState();
    this.isFlipped = false;
    this.highlightedSquares = new Set<string>();
    this.lastMove = null;
    this.moves = [];
    this.initialSfen = '';
    this.pixelRatio = (window as any).devicePixelRatio || 1;
    this.logicalWidth = 0;
    this.logicalHeight = 0;
    this.theme = { ...DEFAULT_THEME };
    this.options = { showHands: true };
    this.layout = null;
    this.arrows = [];
    this.circles = [];
    this.animFrames = [];
    this.animOptions = { loop: false, defaultDuration: 1000, autoPlay: true };
    this.animState = AnimationPlayState.STOPPED;
    this.animCurrentFrame = 0;
    this.animTimerId = 0;
    this.animFrameStartTime = 0;

    this.resizeHandler = () => this.resize();
    this.resize();
    window.addEventListener('resize', this.resizeHandler);
  }

  resize(): void {
    const rect = this.container.getBoundingClientRect();
    const width = Math.max(1, Math.floor(rect.width || this.container.clientWidth || 0));
    const height = Math.max(1, Math.floor(rect.height || this.container.clientHeight || 0));
    const ratio = (window as any).devicePixelRatio || 1;
    this.pixelRatio = ratio;
    this.logicalWidth = width;
    this.logicalHeight = height;
    this.canvas.width = Math.max(1, Math.floor(width * ratio));
    this.canvas.height = Math.max(1, Math.floor(height * ratio));
    this.canvas.style.width = `${width}px`;
    this.canvas.style.height = `${height}px`;
    this.render();
  }

  setPositionFromSFEN(sfen: string): void {
    this.initialSfen = String(sfen ?? '');
    this.boardState.setPositionFromSFEN(this.initialSfen);
    this.moves = [];
    this.lastMove = null;
    this.render();
  }

  setMoves(usiMoves: readonly string[] | null | undefined): void {
    this.moves = Array.isArray(usiMoves) ? [...usiMoves] : [];
  }

  goTo(ply: number): void {
    if (!Number.isFinite(ply)) return;
    const target = Math.max(0, Math.floor(ply));
    if (target > this.moves.length) return;

    this.lastMove = null;
    this.boardState.setPositionFromSFEN(this.initialSfen);
    for (let i = 0; i < target; i++) {
      const move = this.moves[i];
      if (typeof move === 'string') {
        this.applyUsiMove(move);
      }
    }
    this.render();
  }

  flip(enabled: boolean): void {
    this.isFlipped = Boolean(enabled);
    this.render();
  }

  setTheme(theme: Partial<BoardTheme> | null | undefined): void {
    if (!theme) return;
    this.theme = { ...this.theme, ...theme };
    this.render();
  }

  setOptions(options: Partial<RendererOptions> | null | undefined): void {
    if (!options) return;
    this.options = { ...this.options, ...options };
    this.render();
  }

  highlightSquares(squares: Iterable<number> | null | undefined): void {
    this.highlightedSquares.clear();
    if (squares) {
      for (const sq of squares) {
        const value = Number(sq);
        if (!Number.isFinite(value)) continue;
        const file = Math.floor(value / 9) + 1;
        const rank = (value % 9) + 1;
        this.highlightedSquares.add(`${file},${rank}`);
      }
    }
    this.render();
  }

  private applyUsiMove(usiMove: string): void {
    if (!usiMove || usiMove.length < 4) return;

    const move: ParsedMove = {
      from: null,
      to: { file: 0, rank: 0 },
      piece: PieceType.PAWN,
      promote: false,
    };

    if (usiMove.includes('*')) {
      const [pieceChar, toPart] = usiMove.split('*');
      const toStr = toPart ?? '';
      move.to.file = Number.parseInt(toStr[0] ?? '0', 10);
      move.to.rank = ((toStr[1]?.toLowerCase().charCodeAt(0) as number) ?? 'a'.charCodeAt(0)) - 96;
      const dropPieceMap: Record<string, PieceType> = {
        P: PieceType.PAWN,
        L: PieceType.LANCE,
        N: PieceType.KNIGHT,
        S: PieceType.SILVER,
        G: PieceType.GOLD,
        B: PieceType.BISHOP,
        R: PieceType.ROOK,
      };
      move.piece = dropPieceMap[pieceChar as keyof typeof dropPieceMap] ?? PieceType.PAWN;
    } else {
      const fromFile = Number.parseInt(usiMove[0] ?? '0', 10);
      const fromRank = ((usiMove[1]?.toLowerCase().charCodeAt(0) as number) ?? 'a'.charCodeAt(0)) - 96;
      const toFile = Number.parseInt(usiMove[2] ?? '0', 10);
      const toRank = ((usiMove[3]?.toLowerCase().charCodeAt(0) as number) ?? 'a'.charCodeAt(0)) - 96;
      move.from = { file: fromFile, rank: fromRank };
      move.to = { file: toFile, rank: toRank };
      move.promote = usiMove.length > 4 && usiMove[4] === '+';
      const piece = this.boardState.getPiece(fromFile, fromRank);
      if (piece) move.piece = piece.type;
    }

    const turn = this.boardState.getTurn();
    const idxFrom = move.from ? { r: move.from.rank - 1, c: 9 - move.from.file } : null;
    const idxTo = { r: move.to.rank - 1, c: 9 - move.to.file };
    if (idxTo.r < 0 || idxTo.r >= 9 || idxTo.c < 0 || idxTo.c >= 9) {
      return;
    }

    if (!move.from) {
      const pt = move.piece;
      this.boardState.board[idxTo.r][idxTo.c] = { type: pt, color: turn } as BoardPiece;
      const hands = this.boardState.getHands(turn);
      const current = (hands.get(pt) ?? 0) - 1;
      if (current > 0) {
        hands.set(pt, current);
      } else {
        hands.delete(pt);
      }
    } else if (idxFrom && idxFrom.r >= 0 && idxFrom.r < 9 && idxFrom.c >= 0 && idxFrom.c < 9) {
      const movingPiece = this.boardState.getPiece(move.from.file, move.from.rank);
      if (!movingPiece) {
        this.lastMove = move;
        return;
      }
      const captured = this.boardState.getPiece(move.to.file, move.to.rank);
      if (captured && captured.color !== movingPiece.color) {
        const capturedBase = this.boardState.demoteType(captured.type);
        const hands = this.boardState.getHands(movingPiece.color);
        hands.set(capturedBase, (hands.get(capturedBase) ?? 0) + 1);
      }
      const newType = move.promote ? this.boardState.promoteType(movingPiece.type) : movingPiece.type;
      this.boardState.board[idxFrom.r][idxFrom.c] = null;
      this.boardState.board[idxTo.r][idxTo.c] = { type: newType, color: movingPiece.color } as BoardPiece;
    }

    this.boardState.currentTurn = turn === Color.BLACK ? Color.WHITE : Color.BLACK;
    this.lastMove = move;
  }

  private render(): void {
    const ratio = this.pixelRatio || 1;
    const width = this.logicalWidth || Math.max(1, Math.floor(this.canvas.width / ratio));
    const height = this.logicalHeight || Math.max(1, Math.floor(this.canvas.height / ratio));
    this.ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    (this.ctx as any).imageSmoothingEnabled = false;

    const sideMarginCells = 2;
    const verticalMarginCells = this.options.showHands ? 3 : 0.5;
    const cellSizeByWidth = Math.floor(width / (9 + sideMarginCells));
    const cellSizeByHeight = Math.floor(height / (9 + verticalMarginCells));
    const cellSize = Math.max(1, Math.min(cellSizeByWidth, cellSizeByHeight));
    const boardSize = cellSize * 9;
    const offsetX = Math.floor((width - boardSize) / 2);
    const offsetY = Math.floor((height - boardSize) / 2);

    this.ctx.clearRect(0, 0, width, height);
    this.ctx.fillStyle = this.theme.boardColor;
    this.ctx.fillRect(0, 0, width, height);

    this.ctx.strokeStyle = this.theme.gridColor;
    const gridLineWidth = Math.max(1 / ratio, 0.5 / ratio);
    this.ctx.lineWidth = gridLineWidth;
    for (let i = 0; i <= 9; i++) {
      this.ctx.beginPath();
      this.ctx.moveTo(offsetX + i * cellSize, offsetY);
      this.ctx.lineTo(offsetX + i * cellSize, offsetY + boardSize);
      this.ctx.stroke();
      this.ctx.beginPath();
      this.ctx.moveTo(offsetX, offsetY + i * cellSize);
      this.ctx.lineTo(offsetX + boardSize, offsetY + i * cellSize);
      this.ctx.stroke();
    }

    if (this.lastMove?.to) {
      this.ctx.fillStyle = this.theme.lastMoveColor;
      if (this.lastMove.from) {
        const fromFile = this.isFlipped ? 10 - this.lastMove.from.file : this.lastMove.from.file;
        const fromRank = this.isFlipped ? 10 - this.lastMove.from.rank : this.lastMove.from.rank;
        const fromX = offsetX + (9 - fromFile) * cellSize;
        const fromY = offsetY + (fromRank - 1) * cellSize;
        this.ctx.fillRect(fromX, fromY, cellSize, cellSize);
      }
      const toFile = this.isFlipped ? 10 - this.lastMove.to.file : this.lastMove.to.file;
      const toRank = this.isFlipped ? 10 - this.lastMove.to.rank : this.lastMove.to.rank;
      const toX = offsetX + (9 - toFile) * cellSize;
      const toY = offsetY + (toRank - 1) * cellSize;
      this.ctx.fillStyle = 'rgba(50, 200, 50, 0.5)';
      this.ctx.fillRect(toX, toY, cellSize, cellSize);
    }

    this.ctx.font = `${Math.floor(cellSize * 0.7)}px serif`;
    this.ctx.textAlign = 'center';
    this.ctx.textBaseline = 'middle';
    for (let rank = 1; rank <= 9; rank++) {
      for (let file = 1; file <= 9; file++) {
        const displayFile = this.isFlipped ? 10 - file : file;
        const displayRank = this.isFlipped ? 10 - rank : rank;
        const x = offsetX + (9 - displayFile) * cellSize + cellSize / 2;
        const y = offsetY + (displayRank - 1) * cellSize + cellSize / 2;

        if (this.highlightedSquares.has(`${file},${rank}`)) {
          this.ctx.fillStyle = this.theme.highlightColor;
          this.ctx.fillRect(x - cellSize / 2, y - cellSize / 2, cellSize, cellSize);
        }

        const piece = this.boardState.getPiece(file, rank);
        if (!piece) continue;

        this.ctx.save();
        const shouldRotatePiece = this.isFlipped
          ? piece.color === Color.BLACK
          : piece.color === Color.WHITE;
        if (shouldRotatePiece) {
          this.ctx.translate(x, y);
          this.ctx.rotate(Math.PI);
          this.ctx.translate(-x, -y);
        }
        this.ctx.fillStyle =
          piece.color === Color.BLACK ? this.theme.blackPieceColor : this.theme.whitePieceColor;
        this.ctx.fillText(this.getPieceChar(piece.type), x, y);
        this.ctx.restore();
      }
    }

    this.ctx.font = `${Math.floor(cellSize * 0.3)}px sans-serif`;
    this.ctx.fillStyle = this.theme.coordinateColor;
    for (let i = 1; i <= 9; i++) {
      const fileNum = this.isFlipped ? i : 10 - i;
      this.ctx.fillText(String(fileNum), offsetX + (i - 0.5) * cellSize, offsetY - cellSize * 0.3);
      const rankNum = this.isFlipped ? 10 - i : i;
      this.ctx.fillText(
        this.toKanjiNumber(rankNum),
        offsetX + boardSize + cellSize * 0.3,
        offsetY + (i - 0.5) * cellSize,
      );
    }

    if (!this.options.showHands) {
      this.layout = {
        offsetX, offsetY, cellSize, boardSize,
        regionW: 0, regionH: 0,
        regionXSente: 0, regionXGote: 0,
        senteRegionY: 0, goteRegionY: 0,
        width, height,
      };
      this.renderArrows();
      this.renderCircles();
      return;
    }

    const drawHandsRegion = (xLeft: number, yTop: number, regionW: number, regionH: number): void => {
      this.ctx.fillStyle = 'rgba(0,0,0,0.08)';
      this.ctx.fillRect(xLeft, yTop, regionW, regionH);
      this.ctx.strokeStyle = '#d8c7a0';
      this.ctx.lineWidth = gridLineWidth;
      this.ctx.strokeRect(xLeft, yTop, regionW, regionH);
    };

    const drawHandTiles = (color: Color, xLeft: number, yTop: number, regionW: number, regionH: number): void => {
      const orderSente: PieceType[] = [
        PieceType.ROOK,
        PieceType.BISHOP,
        PieceType.GOLD,
        PieceType.SILVER,
        PieceType.KNIGHT,
        PieceType.LANCE,
        PieceType.PAWN,
      ];
      const orderGote: PieceType[] = [
        PieceType.PAWN,
        PieceType.LANCE,
        PieceType.KNIGHT,
        PieceType.SILVER,
        PieceType.GOLD,
        PieceType.BISHOP,
        PieceType.ROOK,
      ];
      const order = color === Color.WHITE ? orderGote : orderSente;
      const hands = this.boardState.getHands(color);
      const slotW = Math.floor(regionW / order.length);
      const centerY = yTop + Math.floor(regionH / 2);
      const glyphSize = Math.max(14, Math.floor(cellSize * 0.7));
      const countSize = Math.max(9, Math.floor(cellSize * 0.32));
      this.ctx.textBaseline = 'middle';
      this.ctx.fillStyle = '#111';

      for (let i = 0; i < order.length; i++) {
        const pt = order[i];
        const cnt = hands.get(pt) ?? 0;
        if (cnt <= 0) continue;
        const centerX = xLeft + Math.floor(i * slotW + slotW / 2);
        this.ctx.save();
        this.ctx.font = `${glyphSize}px serif`;
        this.ctx.textAlign = 'center';
        this.ctx.translate(centerX, centerY + Math.floor(glyphSize * 0.06));
        if (color === Color.WHITE) {
          this.ctx.rotate(Math.PI);
        }
        this.ctx.fillText(this.getPieceChar(pt), 0, 0);
        if (cnt > 1) {
          this.ctx.font = `${countSize}px sans-serif`;
          this.ctx.textAlign = 'left';
          this.ctx.textBaseline = 'alphabetic';
          const offsetX = Math.floor(slotW * 0.28);
          const offsetY = Math.floor(regionH * 0.35);
          this.ctx.fillText(String(cnt), offsetX, offsetY);
        }
        this.ctx.restore();
      }
    };

    const mmToPx = (mm: number): number => Math.round((96 / 25.4) * mm);
    const regionW = Math.floor(cellSize * 8.0);
    const regionH = Math.floor(cellSize * 0.9);
    const regionXSente = offsetX + boardSize - regionW;
    const regionXGote = offsetX;
    const bandGap = Math.max(0, Math.floor(cellSize * 0.8) - mmToPx(3));
    const goteRegionY = Math.max(0, offsetY - bandGap - regionH);
    const senteRegionY = Math.min(height - regionH, offsetY + boardSize + bandGap);

    drawHandsRegion(regionXGote, goteRegionY, regionW, regionH);
    drawHandsRegion(regionXSente, senteRegionY, regionW, regionH);
    drawHandTiles(Color.WHITE, regionXGote, goteRegionY, regionW, regionH);
    drawHandTiles(Color.BLACK, regionXSente, senteRegionY, regionW, regionH);

    if (this.lastMove && !this.lastMove.from) {
      const mover = this.boardState.getTurn() === Color.BLACK ? Color.WHITE : Color.BLACK;
      const orderSente: PieceType[] = [
        PieceType.ROOK,
        PieceType.BISHOP,
        PieceType.GOLD,
        PieceType.SILVER,
        PieceType.KNIGHT,
        PieceType.LANCE,
        PieceType.PAWN,
      ];
      const orderGote: PieceType[] = [
        PieceType.PAWN,
        PieceType.LANCE,
        PieceType.KNIGHT,
        PieceType.SILVER,
        PieceType.GOLD,
        PieceType.BISHOP,
        PieceType.ROOK,
      ];
      const order = mover === Color.WHITE ? orderGote : orderSente;
      const slotW = Math.floor(regionW / order.length);
      const centerY =
        mover === Color.WHITE ? goteRegionY + Math.floor(regionH / 2) : senteRegionY + Math.floor(regionH / 2);
      const pt = this.lastMove.piece;
      const idx = order.indexOf(pt);
      if (idx >= 0) {
        const centerX =
          mover === Color.WHITE
            ? regionXGote + Math.floor(idx * slotW + slotW / 2)
            : regionXSente + Math.floor(idx * slotW + slotW / 2);
        const w = Math.floor(slotW * 0.9);
        const h = Math.floor(regionH * 0.9);
        this.ctx.fillStyle = this.theme.lastMoveColor;
        this.ctx.fillRect(centerX - Math.floor(w / 2), centerY - Math.floor(h / 2), w, h);
      }
    }

    this.layout = {
      offsetX, offsetY, cellSize, boardSize,
      regionW, regionH,
      regionXSente, regionXGote,
      senteRegionY, goteRegionY,
      width, height,
    };
    this.renderArrows();
    this.renderCircles();
  }

  private resolveSquareCenter(spec: SquareSpec): { x: number; y: number } | null {
    if (!this.layout) return null;
    const resolved = parseSquareSpec(spec);
    if (!resolved) return null;
    const L = this.layout;

    if (resolved.kind === 'board') {
      const displayFile = this.isFlipped ? 10 - resolved.file : resolved.file;
      const displayRank = this.isFlipped ? 10 - resolved.rank : resolved.rank;
      const x = L.offsetX + (9 - displayFile) * L.cellSize + L.cellSize / 2;
      const y = L.offsetY + (displayRank - 1) * L.cellSize + L.cellSize / 2;
      return { x, y };
    }

    // Hand piece slot
    const orderSente: PieceType[] = [
      PieceType.ROOK, PieceType.BISHOP, PieceType.GOLD,
      PieceType.SILVER, PieceType.KNIGHT, PieceType.LANCE, PieceType.PAWN,
    ];
    const orderGote: PieceType[] = [
      PieceType.PAWN, PieceType.LANCE, PieceType.KNIGHT,
      PieceType.SILVER, PieceType.GOLD, PieceType.BISHOP, PieceType.ROOK,
    ];
    const color = resolved.color;
    const order = color === Color.WHITE ? orderGote : orderSente;
    const idx = order.indexOf(resolved.pieceType);
    if (idx < 0 || L.regionW === 0) return null;
    const slotW = Math.floor(L.regionW / order.length);
    const regionX = color === Color.WHITE ? L.regionXGote : L.regionXSente;
    const regionY = color === Color.WHITE ? L.goteRegionY : L.senteRegionY;
    const x = regionX + Math.floor(idx * slotW + slotW / 2);
    const y = regionY + Math.floor(L.regionH / 2);
    return { x, y };
  }

  setArrows(arrows: ArrowAnnotation[]): void {
    this.arrows = Array.isArray(arrows) ? [...arrows] : [];
    this.render();
  }

  setCircles(circles: CircleAnnotation[]): void {
    this.circles = Array.isArray(circles) ? [...circles] : [];
    this.render();
  }

  clearAnnotations(): void {
    this.arrows = [];
    this.circles = [];
    this.render();
  }

  animate(frames: AnimationFrame[], options?: AnimationOptions): void {
    this.stopAnimation();
    if (!frames || frames.length === 0) return;
    this.animFrames = [...frames];
    this.animOptions = {
      loop: options?.loop ?? false,
      defaultDuration: options?.defaultDuration ?? 1000,
      autoPlay: options?.autoPlay ?? true,
    };
    this.animCurrentFrame = 0;
    this.applyAnimationFrame(0);
    if (this.animOptions.autoPlay) {
      this.animState = AnimationPlayState.PLAYING;
      this.animFrameStartTime = performance.now();
      this.scheduleNextFrame();
    } else {
      this.animState = AnimationPlayState.PAUSED;
    }
  }

  pauseAnimation(): void {
    if (this.animState === AnimationPlayState.PLAYING) {
      this.animState = AnimationPlayState.PAUSED;
      if (this.animTimerId) {
        cancelAnimationFrame(this.animTimerId);
        this.animTimerId = 0;
      }
    }
  }

  resumeAnimation(): void {
    if (this.animState === AnimationPlayState.PAUSED && this.animFrames.length > 0) {
      this.animState = AnimationPlayState.PLAYING;
      this.animFrameStartTime = performance.now();
      this.scheduleNextFrame();
    }
  }

  stopAnimation(): void {
    if (this.animTimerId) {
      cancelAnimationFrame(this.animTimerId);
      this.animTimerId = 0;
    }
    this.animState = AnimationPlayState.STOPPED;
    this.animCurrentFrame = 0;
    this.animFrames = [];
  }

  goToFrame(index: number): void {
    if (index < 0 || index >= this.animFrames.length) return;
    if (this.animState === AnimationPlayState.PLAYING) {
      this.pauseAnimation();
    }
    this.animCurrentFrame = index;
    this.applyAnimationFrame(index);
  }

  private applyAnimationFrame(index: number): void {
    if (index < 0 || index >= this.animFrames.length) return;
    const frame = this.animFrames[index];

    if (frame.sfen !== undefined) {
      this.initialSfen = frame.sfen;
      this.boardState.setPositionFromSFEN(frame.sfen);
      this.lastMove = null;
      this.moves = [];
    }

    if (frame.ply !== undefined && this.moves.length > 0) {
      this.boardState.setPositionFromSFEN(this.initialSfen);
      this.lastMove = null;
      for (let i = 0; i < frame.ply && i < this.moves.length; i++) {
        const move = this.moves[i];
        if (typeof move === 'string') {
          this.applyUsiMove(move);
        }
      }
    }

    if (frame.arrows !== undefined) {
      this.arrows = [...frame.arrows];
    }
    if (frame.circles !== undefined) {
      this.circles = [...frame.circles];
    }
    if (frame.highlights !== undefined) {
      this.highlightedSquares.clear();
      for (const sq of frame.highlights) {
        const value = Number(sq);
        if (!Number.isFinite(value)) continue;
        const file = Math.floor(value / 9) + 1;
        const rank = (value % 9) + 1;
        this.highlightedSquares.add(`${file},${rank}`);
      }
    }

    this.render();
  }

  private scheduleNextFrame(): void {
    if (this.animState !== AnimationPlayState.PLAYING) return;
    this.animTimerId = requestAnimationFrame((now: number) => {
      if (this.animState !== AnimationPlayState.PLAYING) return;
      const currentFrame = this.animFrames[this.animCurrentFrame];
      const duration = currentFrame?.duration ?? this.animOptions.defaultDuration ?? 1000;
      const elapsed = now - this.animFrameStartTime;
      if (elapsed >= duration) {
        const nextFrame = this.animCurrentFrame + 1;
        if (nextFrame >= this.animFrames.length) {
          if (this.animOptions.loop) {
            this.animCurrentFrame = 0;
            this.applyAnimationFrame(0);
            this.animFrameStartTime = now;
            this.scheduleNextFrame();
          } else {
            this.animState = AnimationPlayState.STOPPED;
          }
        } else {
          this.animCurrentFrame = nextFrame;
          this.applyAnimationFrame(nextFrame);
          this.animFrameStartTime = now;
          this.scheduleNextFrame();
        }
      } else {
        this.scheduleNextFrame();
      }
    });
  }

  private renderArrows(): void {
    if (this.arrows.length === 0 || !this.layout) return;
    const L = this.layout;
    const ctx = this.ctx;
    const baseLineWidth = L.cellSize * 0.15;

    for (const arrow of this.arrows) {
      const fromPt = this.resolveSquareCenter(arrow.from);
      const toPt = this.resolveSquareCenter(arrow.to);
      if (!fromPt || !toPt) continue;

      const color = arrow.color ?? DEFAULT_ARROW_COLOR;
      const widthMult = arrow.width ?? 1.0;
      const lineWidth = baseLineWidth * widthMult;

      const dx = toPt.x - fromPt.x;
      const dy = toPt.y - fromPt.y;
      const length = Math.sqrt(dx * dx + dy * dy);
      if (length < 1) continue;

      const ux = dx / length;
      const uy = dy / length;

      const headLength = lineWidth * 3.5;
      const headWidth = lineWidth * 2.5;

      // Offset start/end slightly from square centers
      const startOffset = L.cellSize * 0.15;
      const endOffset = L.cellSize * 0.1;
      const startX = fromPt.x + ux * startOffset;
      const startY = fromPt.y + uy * startOffset;
      const tipX = toPt.x - ux * endOffset;
      const tipY = toPt.y - uy * endOffset;

      const adjDx = tipX - startX;
      const adjDy = tipY - startY;
      const adjLen = Math.sqrt(adjDx * adjDx + adjDy * adjDy);
      if (adjLen < headLength) continue;
      const adjUx = adjDx / adjLen;
      const adjUy = adjDy / adjLen;
      const shaftEndX = tipX - adjUx * headLength;
      const shaftEndY = tipY - adjUy * headLength;

      const perpX = -adjUy;
      const perpY = adjUx;

      ctx.save();
      // Shaft
      ctx.strokeStyle = color;
      ctx.lineWidth = lineWidth;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      ctx.beginPath();
      ctx.moveTo(startX, startY);
      ctx.lineTo(shaftEndX, shaftEndY);
      ctx.stroke();
      // Arrowhead
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.moveTo(tipX, tipY);
      ctx.lineTo(shaftEndX + perpX * headWidth / 2, shaftEndY + perpY * headWidth / 2);
      ctx.lineTo(shaftEndX - perpX * headWidth / 2, shaftEndY - perpY * headWidth / 2);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
    }
  }

  private renderCircles(): void {
    if (this.circles.length === 0 || !this.layout) return;
    const L = this.layout;
    const ctx = this.ctx;
    const baseStrokeWidth = L.cellSize * 0.06;

    for (const circle of this.circles) {
      const center = this.resolveSquareCenter(circle.square);
      if (!center) continue;

      const color = circle.color ?? DEFAULT_CIRCLE_COLOR;
      const widthMult = circle.width ?? 1.0;
      const strokeWidth = baseStrokeWidth * widthMult;

      const resolved = parseSquareSpec(circle.square);
      const radiusFactor = resolved?.kind === 'hand' ? 0.30 : 0.40;
      const radius = L.cellSize * radiusFactor;

      ctx.save();
      ctx.strokeStyle = color;
      ctx.lineWidth = strokeWidth;
      ctx.beginPath();
      ctx.arc(center.x, center.y, radius, 0, Math.PI * 2);
      ctx.stroke();
      ctx.restore();
    }
  }

  private getPieceChar(type: PieceType): string {
    const pieceChars: Record<number, string> = {
      [PieceType.PAWN]: '歩',
      [PieceType.LANCE]: '香',
      [PieceType.KNIGHT]: '桂',
      [PieceType.SILVER]: '銀',
      [PieceType.GOLD]: '金',
      [PieceType.BISHOP]: '角',
      [PieceType.ROOK]: '飛',
      [PieceType.KING]: '王',
      [PieceType.PRO_PAWN]: 'と',
      [PieceType.PRO_LANCE]: '杏',
      [PieceType.PRO_KNIGHT]: '圭',
      [PieceType.PRO_SILVER]: '全',
      [PieceType.HORSE]: '馬',
      [PieceType.DRAGON]: '龍',
    };
    return pieceChars[type] ?? '';
  }

  private toKanjiNumber(value: number): string {
    const kanjiNumbers = ['', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    return kanjiNumbers[value] ?? '';
  }

  dispose(): void {
    this.stopAnimation();
    window.removeEventListener('resize', this.resizeHandler);
    if (this.canvas.parentNode === this.container) {
      this.container.removeChild(this.canvas);
    }
  }
}

class ShogiBoardAdapterImpl implements LiveBoardAdapter {
  [key: string]: unknown;
  private renderer: ShogiBoardRenderer | null = null;
  currentPly: number | undefined;

  mount(target: Element): void {
    const element = target instanceof HTMLElement ? target : null;
    if (!element) {
      throw new Error('ShogiBoardAdapter.mount expects an HTMLElement');
    }
    this.renderer?.dispose();
    this.renderer = new ShogiBoardRenderer(element);
  }

  resize(): void {
    this.renderer?.resize();
  }

  setPositionFromSFEN(sfen: string): void {
    this.renderer?.setPositionFromSFEN(sfen);
  }

  setMoves(moves: readonly string[]): void {
    this.renderer?.setMoves(moves);
  }

  goTo(ply: number): void {
    this.renderer?.goTo(ply);
    this.currentPly = ply;
  }

  flip(enabled: boolean): void {
    this.renderer?.flip(enabled);
  }

  highlightSquares(indices: Iterable<number>): void {
    this.renderer?.highlightSquares(indices);
  }

  setTheme(theme: Partial<BoardTheme>): void {
    this.renderer?.setTheme(theme);
  }

  setOptions(options: Partial<{ showHands: boolean }>): void {
    this.renderer?.setOptions(options);
  }

  setArrows(arrows: ArrowAnnotation[]): void {
    this.renderer?.setArrows(arrows);
  }

  setCircles(circles: CircleAnnotation[]): void {
    this.renderer?.setCircles(circles);
  }

  clearAnnotations(): void {
    this.renderer?.clearAnnotations();
  }

  animate(frames: AnimationFrame[], options?: AnimationOptions): void {
    this.renderer?.animate(frames, options);
  }

  pauseAnimation(): void {
    this.renderer?.pauseAnimation();
  }

  resumeAnimation(): void {
    this.renderer?.resumeAnimation();
  }

  stopAnimation(): void {
    this.renderer?.stopAnimation();
  }

  goToFrame(index: number): void {
    this.renderer?.goToFrame(index);
  }

  destroy(): void {
    this.dispose();
  }

  dispose(): void {
    this.renderer?.dispose();
    this.renderer = null;
    this.currentPly = undefined;
  }
}

interface ShogiBoardWindow extends Window {
  ShogiBoardAdapter?: new () => ShogiBoardAdapterImpl;
  ShogiBoardRenderer?: typeof ShogiBoardRenderer;
}

export function installShogiBoardGlobals(owner: ShogiBoardWindow = window as ShogiBoardWindow) {
  owner.ShogiBoardAdapter = ShogiBoardAdapterImpl;
  owner.ShogiBoardRenderer = ShogiBoardRenderer;
  return { ShogiBoardAdapter: ShogiBoardAdapterImpl, ShogiBoardRenderer };
}

export { ShogiBoardAdapterImpl as ShogiBoardAdapter, ShogiBoardRenderer };
export type { ArrowAnnotation, CircleAnnotation, AnimationFrame, AnimationOptions };
