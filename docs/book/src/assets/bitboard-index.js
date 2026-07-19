(function () {
  function square(file, rank) {
    return (file - 1) * 9 + (rank - 1);
  }

  function mountBoard(ShogiBoardAdapter, id, sfen, highlights) {
    const root = document.getElementById(id);
    if (!root) return null;
    const board = new ShogiBoardAdapter();
    board.mount(root);
    board.setOptions({ showHands: false });
    board.setPositionFromSFEN(sfen);
    board.highlightSquares(highlights);
    board.goTo(0);
    return board;
  }

  function initAndDemo() {
    if (!window.RShogiBoard) return;
    const api = window.RShogiBoard.installShogiBoardGlobals(window);
    const ShogiBoardAdapter = api.ShogiBoardAdapter;

    const allPawnSquares = [];
    const blackPieceSquares = [];
    const blackPawnSquares = [];
    for (let file = 1; file !== 10; file += 1) {
      allPawnSquares.push(square(file, 3), square(file, 7));
      blackPawnSquares.push(square(file, 7));
      blackPieceSquares.push(square(file, 7));
    }
    const backRankPieces = [
      [8, 8], [2, 8],
      [9, 9], [8, 9], [7, 9], [6, 9], [5, 9], [4, 9], [3, 9], [2, 9], [1, 9],
    ];
    for (const item of backRankPieces) {
      blackPieceSquares.push(square(item[0], item[1]));
    }

    window.basicsLiveAndPawns = mountBoard(
      ShogiBoardAdapter,
      'basics-live-and-pawns',
      '9/9/ppppppppp/9/9/9/PPPPPPPPP/9/9 b - 1',
      allPawnSquares
    );
    window.basicsLiveAndBlack = mountBoard(
      ShogiBoardAdapter,
      'basics-live-and-black',
      '9/9/9/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1',
      blackPieceSquares
    );
    window.basicsLiveAndResult = mountBoard(
      ShogiBoardAdapter,
      'basics-live-and-result',
      '9/9/9/9/9/9/PPPPPPPPP/9/9 b - 1',
      blackPawnSquares
    );
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initAndDemo, { once: true });
  } else {
    initAndDemo();
  }
})();
