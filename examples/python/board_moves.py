from rsshogi.core import Board

board = Board()

print("turn:", board.turn, "ply:", board.game_ply)

legal = board.legal_moves()
print("legal move count:", len(legal))
print("first five legal moves:", [mv.to_usi() for mv in legal[:5]])

if legal:
    board.apply_move(legal[0])

pseudo = board.pseudo_legal_moves()
print("pseudo-legal move count:", len(pseudo))

last_move = board.last_move()
if last_move:
    print("last move:", last_move.to_usi())
print("current sfen:", board.to_sfen())
print("repetition:", board.repetition_state().to_usi())
print("is repetition:", board.is_repetition())
