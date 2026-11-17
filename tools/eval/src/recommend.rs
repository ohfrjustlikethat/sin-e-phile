//! Phase 17, E1 and E2: the item-item matrix against popularity, on MovieLens users it never
//! learned from.
//!
//! From the local `ml-25m.zip`: users are split by a hash of their id
//! (`sinephile_recommend::split`): 80% build the matrix, 10% tune its two numbers
//! (`--tune`), 10% judge it. A fifth of each judged user's likes is held out; the rest is
//! what the recommender is shown. Recall@20 is the share of the held-out likes it finds in
//! its 20, out of at most 20 (`hits / min(20, held out)`), averaged over users; popularity
//! gets the same 20 slots, filled with the most-liked titles the user has not liked.
//!
//! E1 asks `recall_model / recall_popularity - 1 > 0.40`. E2 (coverage) is the share of the
//! matrix's titles that reach at least one judged user's 20.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use sinephile_catalogue::movielens;
use sinephile_recommend::cf::{self, Likes, Matrix, Rating, Score};
use sinephile_recommend::split::{self, Part};

use crate::error::EvalError;

const K: usize = 20;
/// Neighbours kept per title, as the app keeps them.
const TOP: usize = 50;
/// A judged user must show the recommender at least this many likes.
const MIN_SHOWN: usize = 5;

/// The score chosen on the tuning users (`eval recommend --tune`, docs/eval-results.md): the
/// best recall whose coverage clears E2's 15%. Cosine (0.5) found more but reached 10%; 0.65
/// gives up 3% of recall to reach 16.6%. Shrinkage did not help at this alpha.
pub const CHOSEN: Score = sinephile_recommend::cf::CHOSEN;

struct Judged {
    shown: Vec<u32>,
    held: Vec<u32>,
}

fn missing(e: impl std::fmt::Display) -> EvalError {
    EvalError::Missing(e.to_string())
}

/// `--tune` sweeps `alphas` x `shrinks` (by default a coarse grid; `--alpha 0.6,0.7` and
/// `--shrink 0,2` narrow it).
pub fn run(
    data_dir: &Path,
    tune: bool,
    alphas: &[f32],
    shrinks: &[f32],
) -> Result<bool, EvalError> {
    let path = data_dir.join("datasets").join("ml-25m.zip");
    let judge = if tune { Part::Tune } else { Part::Test };
    println!(
        "recommend — the item-item matrix against popularity, Recall@{K}, {} users\n",
        if tune { "tuning" } else { "test" }
    );

    let started = Instant::now();
    let mut building: Vec<Rating> = Vec::new();
    let mut judged: HashMap<u32, Judged> = HashMap::new();
    let read = movielens::ratings(&path, |user, item, stars| {
        if stars < cf::LIKED {
            return;
        }
        match split::part(user) {
            Part::Build => building.push(Rating { user, item, stars }),
            p if p == judge => {
                let j = judged.entry(user).or_insert_with(|| Judged {
                    shown: Vec::new(),
                    held: Vec::new(),
                });
                if split::held_out(user, item) {
                    j.held.push(item);
                } else {
                    j.shown.push(item);
                }
            }
            _ => {}
        }
    })
    .map_err(missing)?;
    let read_secs = started.elapsed().as_secs_f64();

    let started = Instant::now();
    let likes = Likes::from_ratings(building, cf::MIN_LIKES);
    let index_secs = started.elapsed().as_secs_f64();
    println!(
        "  read {read} ratings in {read_secs:.1} s; indexed {} likes by {} people over {} titles in {index_secs:.1} s",
        likes.likes(),
        likes.people(),
        likes.items()
    );

    let popular: Vec<u32> = likes.popularity().into_iter().map(|(id, _)| id).collect();
    let in_matrix: HashSet<u32> = popular.iter().copied().collect();
    // Judged users with enough shown, and something held out that could be found at all.
    let users: Vec<(Vec<u32>, Vec<u32>)> = judged
        .into_values()
        .filter_map(|j| {
            let held: Vec<u32> = j
                .held
                .into_iter()
                .filter(|i| in_matrix.contains(i))
                .collect();
            (j.shown.len() >= MIN_SHOWN && !held.is_empty()).then_some((j.shown, held))
        })
        .collect();
    println!("  judged on {} users\n", users.len());

    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let base = popularity_recall(&popular, &users);
    println!("  popularity            Recall@{K} {base:.4}");

    let grid: Vec<Score> = if tune {
        alphas
            .iter()
            .flat_map(|&alpha| shrinks.iter().map(move |&shrink| Score { alpha, shrink }))
            .collect()
    } else {
        vec![CHOSEN]
    };
    let mut best: Option<(f64, Score)> = None;
    let mut passed = false;
    for score in grid {
        let started = Instant::now();
        let matrix = cf::build(&likes, score, TOP, threads);
        let build_secs = started.elapsed().as_secs_f64();
        let (recall, covered) = model_recall(&matrix, &users);
        let lift = recall / base - 1.0;
        println!(
            "  alpha {:.2} shrink {:>4.1}   Recall@{K} {recall:.4}   {:+.1}% against popularity   coverage {:.1}%   built in {build_secs:.1} s on {threads} threads",
            score.alpha,
            score.shrink,
            lift * 100.0,
            covered as f64 / matrix.items() as f64 * 100.0
        );
        if best.is_none_or(|(r, _)| recall > r) {
            best = Some((recall, score));
        }
        passed = lift > 0.40;
    }
    if let (true, Some((recall, score))) = (tune, best) {
        println!(
            "\n  best on the tuning users: alpha {} shrink {} (Recall@{K} {recall:.4})",
            score.alpha, score.shrink
        );
        return Ok(true);
    }
    println!(
        "\n  E1 (more than 40% above popularity): {}",
        if passed { "met" } else { "NOT met" }
    );
    Ok(passed)
}

/// Mean `hits / min(K, held)` of popularity's 20: the most-liked titles not already liked.
fn popularity_recall(popular: &[u32], users: &[(Vec<u32>, Vec<u32>)]) -> f64 {
    let total: f64 = users
        .iter()
        .map(|(shown, held)| {
            let shown: HashSet<u32> = shown.iter().copied().collect();
            let top: Vec<u32> = popular
                .iter()
                .filter(|i| !shown.contains(i))
                .take(K)
                .copied()
                .collect();
            recall_of(&top, held)
        })
        .sum();
    total / users.len() as f64
}

/// The matrix's mean recall, and how many titles reached anyone's 20.
fn model_recall(matrix: &Matrix, users: &[(Vec<u32>, Vec<u32>)]) -> (f64, usize) {
    let mut reached: HashSet<u32> = HashSet::new();
    let total: f64 = users
        .iter()
        .map(|(shown, held)| {
            let top = matrix.recommend(shown, K);
            reached.extend(top.iter().copied());
            recall_of(&top, held)
        })
        .sum();
    (total / users.len() as f64, reached.len())
}

fn recall_of(top: &[u32], held: &[u32]) -> f64 {
    let held: HashSet<u32> = held.iter().copied().collect();
    let hits = top.iter().filter(|i| held.contains(i)).count();
    hits as f64 / K.min(held.len()) as f64
}
