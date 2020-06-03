use std::collections::HashMap;
use crate::types::chess_move::Move;
use crate::position::Position;
use crate::move_generator::MoveGenerator;
use std::cmp;
use crate::evaluator::Evaluator;
use crate::types::bitboard::from_index;
use crate::Engine;
use crate::transposition_table::{TranspositionTable, Entry, EntryFlag};

//An entry in the transposition table

pub(crate) struct Searcher {
    transposition_table: TranspositionTable,
    //We store two killer moves per ply,
    //indexed by killer_moves[depth][0] or killer_moves[depth][0]
    killer_moves: [[Move;2];64],
    //Indexed by history_moves[side2move][from][to]
    history_moves: [[usize;256];256],

    //Stats
    //Counter for the number of nodes searched
    nodes_searched: usize,
    nodes_fail_high_first:usize,
    nodes_fail_high: usize
}

impl Searcher {
    pub fn new() -> Searcher {
        let hasher = ahash::RandomState::new();
        Searcher{
            transposition_table: TranspositionTable::new(),
            killer_moves: [[Move::null(); 2];64],
            history_moves: [[0;256];256],
            nodes_searched: 0,
            nodes_fail_high: 0,
            nodes_fail_high_first: 0
        }
    }


    pub fn get_best_move(&mut self, position: &mut Position, eval: &mut Evaluator, movegen: &MoveGenerator, depth: u8) -> Option<Move> {
        //Iterative deepening
        self.clear_heuristics();
        for d in 1..=depth {
            let best_score = self.alphabeta(position, eval, movegen, d, -isize::MAX, isize::MAX, true);
            //Print PV info
            let ordering_percentage:f64 = if self.nodes_fail_high != 0 { (self.nodes_fail_high_first as f64) / (self.nodes_fail_high as f64) } else { 0.0 };
            println!("score:{} depth: {}, nodes: {}, ordering: {}", best_score, d, self.nodes_searched, ordering_percentage);

            self.clear_search_stats();
        }

        match self.transposition_table.retrieve(position.get_zobrist()){
            Some(entry) => {Some((&entry.move_).to_owned())}
            None => None
        }
    }

    fn alphabeta(&mut self, position: &mut Position, eval: &mut Evaluator, movegen: &MoveGenerator,
                     depth: u8, mut alpha: isize, mut beta: isize, do_null: bool) -> isize {
        self.nodes_searched += 1;

        if depth == 0 {
            return self.quiesce(position, eval, movegen, depth, alpha, beta);
        }

        if let Some(entry) = self.transposition_table.retrieve(position.get_zobrist()) {
            if entry.depth >= depth {
                match entry.flag {
                    EntryFlag::EXACT => {
                        if entry.value < alpha {
                            return alpha;
                        }
                        if entry.value >= beta{
                            return beta;
                        }
                        return entry.value;
                    }
                    EntryFlag::BETA => {
                        if beta <= entry.value {
                            return beta;
                        }
                    }
                    EntryFlag::ALPHA => {
                        if alpha >= entry.value {
                            return alpha;
                        }
                    }
                    EntryFlag::NULL => {}
                }
            }
        }
        //Null move pruning
        if let Some(beta) = self.try_null_move(position, eval, movegen, depth, alpha, beta, do_null){
            return beta;
        }

        let mut moves_and_score = self.get_scored_pseudo_moves(eval, movegen, position, depth);
        let mut best_move = Move::null();
        let mut num_legal_moves = 0;
        let old_alpha = alpha;
        let mut best_score = -isize::MAX;
        let mut search_pv = true;
        for i in 0..moves_and_score.len() {
            //Pick the best move
            Searcher::sort_moves(i, &mut moves_and_score);
            let move_ = moves_and_score[i].1;

            if !movegen.is_move_legal(&move_, position) {
                continue;
            }

            num_legal_moves += 1;
            position.make_move((&move_).to_owned());
            let mut score = 0;
            if search_pv {
                score = -self.alphabeta(position, eval, movegen,
                                        depth - 1, -beta, -alpha, true);
            }else{
                score = -self.alphabeta(position, eval, movegen,
                                        depth - 1, -alpha - 1, -alpha, true);
                if score > alpha  && score < beta {
                    score = -self.alphabeta(position, eval, movegen,
                                            depth - 1, -beta, -alpha, true);
                }
            }

            position.unmake_move();

            if score > best_score {
                best_score = score;
                best_move = move_;

                if score > alpha {
                    if score >= beta {
                        if num_legal_moves == 1 {
                            self.nodes_fail_high_first += 1;
                        }
                        self.nodes_fail_high += 1;
                        //Record new killer moves
                        self.update_killers(depth, (&move_).to_owned());
                        //Beta cutoff, store in transpositon table
                        self.transposition_table.insert(position.get_zobrist(), Entry{
                            key: position.get_zobrist(),
                            flag: EntryFlag::BETA,
                            value: beta,
                            move_,
                            depth
                        });
                        return beta;
                    }
                    search_pv = false;
                    alpha = score;

                    //History heuristic
                    self.update_history_heuristic(depth, &move_);
                }
            }
        }

        if num_legal_moves == 0 {
            return if movegen.in_check(position) {
                //Checkmate
                -99999
            } else {
                //Stalemate
                0
            };
        }

        if alpha != old_alpha {
            //Alpha improvement, record PV
            self.transposition_table.insert(position.get_zobrist(), Entry{
                key: position.get_zobrist(),
                flag: EntryFlag::EXACT,
                value: best_score,
                move_: (&best_move).to_owned(),
                depth
            })
        }else{
            self.transposition_table.insert(position.get_zobrist(), Entry{
                key: position.get_zobrist(),
                flag: EntryFlag::ALPHA,
                value: alpha,
                move_: best_move,
                depth
            })
        }
        alpha
    }


    fn quiesce(&mut self, position: &mut Position, eval: &mut Evaluator, movegen: &MoveGenerator,
                 depth:u8, mut alpha: isize, mut beta: isize) -> isize {
        self.nodes_searched += 1;
        let mut score = eval.evaluate(position, movegen);
        if score >= beta{
            return beta;
        }
        if score > alpha {
            alpha = score;
        }

        let mut best_move = Move::null();
        let mut num_legal_moves = 0;
        let mut moves_and_score = self.get_scored_capture_moves(eval, movegen, position, depth);
        //for (score, move_) in moves_and_score {
        for i in 0..moves_and_score.len() {
            //Pick the best move
            Searcher::sort_moves(i, &mut moves_and_score);
            let move_ = moves_and_score[i].1;

            if !movegen.is_move_legal(&move_, position) {
                continue;
            }

            num_legal_moves += 1;
            position.make_move((&move_).to_owned());
            let score = -self.quiesce(position, eval, movegen,
                                         depth, -beta, -alpha);
            position.unmake_move();

            if score >= beta {
                if num_legal_moves == 1 {
                    self.nodes_fail_high_first += 1;
                }
                self.nodes_fail_high += 1;
                return beta;
            }
            if score > alpha {
                alpha = score;
                best_move = move_;
            }
        }

        alpha
    }

    //Selection sort, swapping at moves_seen with the next best move from moves[current_index:]
    #[inline]
    fn sort_moves(current_index:usize, moves:&mut Vec<(usize, Move)>) {
        let mut best_score = 0;
        let mut best_score_index = current_index;
        for i in current_index..moves.len(){
            let score = moves[i].0;
            if score >= best_score {
                best_score = score;
                best_score_index = i;
            }
        }
        if current_index != best_score_index {
            moves.swap(current_index, best_score_index);
        }
    }

    //Resets heuristics
    fn clear_heuristics(&mut self) {
        for i in 0..self.killer_moves.len() {
            for j in 0..self.killer_moves[i].len() {
                self.killer_moves[i][j] = Move::null();
            }
        }
        for i in 0..self.history_moves.len() {
            for j in 0..self.history_moves[i].len() {
                    self.history_moves[i][j] = 0;
            }
        }
    }

    fn clear_search_stats(&mut self) {
        self.nodes_searched = 0;
        self.nodes_fail_high_first = 0;
        self.nodes_fail_high = 0;
    }

    #[inline]
    fn update_killers(&mut self, depth: u8, move_: Move) {
        if !move_.get_is_capture(){
            if move_ != self.killer_moves[depth as usize][0]
                && move_ != self.killer_moves[depth as usize][1] {
                self.killer_moves[depth as usize][1] = self.killer_moves[depth as usize][0];
                self.killer_moves[depth as usize][0] = move_;
            }
        }
    }

    #[inline]
    fn update_history_heuristic(&mut self, depth: u8, move_:&Move) {
        if !move_.get_is_capture() {
            self.history_moves
                [move_.get_from() as usize]
                [move_.get_to() as usize] += depth as usize;
        }
    }

    #[inline]
    fn get_scored_pseudo_moves(&mut self, eval: &mut Evaluator, movegen: &MoveGenerator, position: &mut Position, depth: u8) -> Vec<(usize, Move)> {
        let mut moves_and_score:Vec<(usize, Move)> = movegen.get_pseudo_moves(position)
            .map(|mv| {
                (eval.score_move(depth,&self.history_moves,&self.killer_moves, position, &mv), mv)
            }).collect();

        //Assign PV/hash moves to usize::MAX
        if let Some(entry) = self.transposition_table.retrieve(position.get_zobrist()) {
            let best_move = &entry.move_;
            for i in 0..moves_and_score.len(){
                if moves_and_score[i].1 == *best_move {
                    moves_and_score[i] = (usize::MAX, moves_and_score[i].1);
                    break;
                }
            }
        }
        moves_and_score
    }

    #[inline]
    fn get_scored_capture_moves(&mut self, eval: &mut Evaluator, movegen: &MoveGenerator, position: &mut Position, depth: u8) -> Vec<(usize, Move)> {
        let mut moves_and_score:Vec<(usize, Move)> = movegen.get_capture_moves(position)
            .map(|mv| {
                (eval.score_move(depth,&self.history_moves,&self.killer_moves, position, &mv), mv)
            }).collect();

        //Assign PV/hash moves to usize::MAX
        if let Some(entry) = self.transposition_table.retrieve(position.get_zobrist()) {
            let best_move = &entry.move_;
            for i in 0..moves_and_score.len(){
                if moves_and_score[i].1 == *best_move {
                    moves_and_score[i] = (usize::MAX, moves_and_score[i].1);
                    break;
                }
            }
        }
        moves_and_score
    }

    #[inline]
    fn try_null_move(&mut self, position: &mut Position, eval: &mut Evaluator, movegen: &MoveGenerator,
                 depth: u8, mut alpha: isize, mut beta: isize, do_null: bool) -> Option<isize> {
        if do_null {
            if depth > 3 && eval.can_do_null_move(position)
                && !movegen.in_check(position) {
                position.make_move(Move::null());
                let nscore = -self.alphabeta(position,eval, movegen,
                                             depth - 3, -beta, -beta + 1, false);
                position.unmake_move();
                if nscore >= beta {
                    return Some(beta);
                }
            }
        }
        None
    }

}