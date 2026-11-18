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
use sinephile_recommend::rank;
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

/// One setting of the ranker to try: its weights and how much variety counts.
#[derive(Debug, Clone, Copy)]
struct Setting {
    weights: rank::Weights,
    diversity: f32,
}

/// What a setting did over the judged users.
#[derive(Default)]
struct Tally {
    recall: f64,
    reached: HashSet<i64>,
    /// Mean intra-list diversity: 1 - cosine, averaged over pairs with vectors.
    ild: f64,
    ild_rails: usize,
    over_caps: usize,
}

/// Mean (1 - cosine) over the pairs of picks that have vectors.
fn intra_list_diversity(
    picks: &[rank::Pick],
    by_id: &HashMap<i64, &rank::Candidate>,
) -> Option<f64> {
    let vectors: Vec<&Vec<f32>> = picks
        .iter()
        .filter_map(|p| by_id.get(&p.id).map(|c| &c.vector))
        .filter(|v| !v.is_empty())
        .collect();
    let mut sum = 0.0;
    let mut pairs = 0usize;
    for i in 0..vectors.len() {
        for j in i + 1..vectors.len() {
            let cos: f32 = vectors[i].iter().zip(vectors[j]).map(|(a, b)| a * b).sum();
            sum += f64::from(1.0 - cos);
            pairs += 1;
        }
    }
    (pairs > 0).then(|| sum / pairs as f64)
}

/// Whether a rail breaks the app's caps (`rank::CAPS`, E3's target): more than allowed per
/// director, franchise or year. Always the app's, whatever `--caps` ranked under, so ranking
/// with looser caps shows the check catching what they let through.
fn breaks_caps(picks: &[rank::Pick], by_id: &HashMap<i64, &rank::Candidate>) -> bool {
    let caps = &rank::CAPS;
    let mut directors: HashMap<i64, usize> = HashMap::new();
    let mut franchises: HashMap<String, usize> = HashMap::new();
    let mut years: HashMap<i64, usize> = HashMap::new();
    for p in picks {
        let Some(c) = by_id.get(&p.id) else { continue };
        for d in &c.directors {
            *directors.entry(*d).or_default() += 1;
        }
        if let Some(f) = &c.franchise {
            *franchises.entry(f.clone()).or_default() += 1;
        }
        if let Some(y) = c.year {
            *years.entry(y).or_default() += 1;
        }
    }
    directors.values().any(|n| *n > caps.per_director)
        || franchises.values().any(|n| *n > caps.per_franchise)
        || years.values().any(|n| *n > caps.per_year)
}

/// The settings `--whole --check` tries over every tuning user: the app's own first, then
/// less weight on popularity and more on variety, the two that widen coverage.
fn coverage_settings() -> Vec<Setting> {
    let mut out = Vec::new();
    for popular in [rank::WEIGHTS.popular, 0.0] {
        for diversity in [rank::DIVERSITY, 0.3, 0.5] {
            out.push(Setting {
                weights: rank::Weights {
                    popular,
                    ..rank::WEIGHTS
                },
                diversity,
            });
        }
    }
    out
}

/// The settings `--whole --tune` tries; without `--tune`, the app's own, then the same by score
/// alone (no variety step), which E3's target is measured against.
fn settings(tune: bool) -> Vec<Setting> {
    if !tune {
        let app = Setting {
            weights: rank::WEIGHTS,
            diversity: rank::DIVERSITY,
        };
        return vec![
            app,
            Setting {
                diversity: 0.0,
                ..app
            },
        ];
    }
    let mut out = Vec::new();
    for loved in [0.5f32, 1.0, 2.0, 4.0] {
        for near in [0.25f32, 0.5, 1.0] {
            for popular in [0.0f32, 0.15, 0.3] {
                for diversity in [0.0f32, 0.15, 0.3] {
                    out.push(Setting {
                        weights: rank::Weights {
                            near,
                            loved,
                            popular,
                            ..rank::WEIGHTS
                        },
                        diversity,
                    });
                }
            }
        }
    }
    out
}

/// Phase 17, E1-E3 and E5 on the whole recommender: the app's own gathering and ranker
/// (`sinephile_taste::hybrid`), judged on MovieLens users the matrix never learned from.
///
/// The matrix is built here from ml-25m's build users only, mapped onto catalogue titles, and
/// handed to the gathering in place of the stored one (which every MovieLens user built).
/// Each judged user's shown likes become a profile: liked titles, taste modes from their
/// vectors in the meaning index, the axes from the catalogue. Their held-out likes are the
/// answer. Popularity gets the same 20 slots: the build users' most-liked titles not shown.
/// `asked`: how many loved titles' neighbours are asked for (the app's
/// `hybrid::LIKED_ASKED` by default); `caps`: the rail's caps (the app's `rank::CAPS` by
/// default). Both are for finding where recall goes, not for the reported run.
/// `check`: the tuning users under the app's own setting, for what a sweep over a few
/// thousand users cannot show (coverage needs them all).
#[allow(clippy::too_many_arguments)]
pub async fn whole(
    data_dir: &Path,
    tune: bool,
    check: bool,
    limit: Option<usize>,
    asked: Option<usize>,
    caps: Option<rank::Caps>,
) -> Result<bool, EvalError> {
    let caps = caps.unwrap_or(rank::CAPS);
    use sinephile_taste::hybrid;
    let path = data_dir.join("datasets").join("ml-25m.zip");
    let judge = if tune || check {
        Part::Tune
    } else {
        Part::Test
    };
    println!(
        "recommend --whole — the hybrid recommender against popularity, Recall@{K}, {} users\n",
        if judge == Part::Tune {
            "tuning"
        } else {
            "test"
        }
    );
    let db = sinephile_persistence::Db::open_in(data_dir).await?;
    let (mut engine, _) = crate::search::engine(data_dir)?;
    let semantic = engine
        .semantic_mut()
        .ok_or_else(|| missing("the meaning index and model are needed"))?;
    let titles = movielens::by_movielens_id(&db).await.map_err(missing)?;

    // The ratings, onto catalogue titles: the build users' likes make the matrix.
    let mut building: Vec<Rating> = Vec::new();
    let mut judged: HashMap<u32, Judged> = HashMap::new();
    movielens::ratings(&path, |user, movie, stars| {
        if stars < cf::LIKED {
            return;
        }
        let Some(item) = titles.get(&movie).and_then(|&id| u32::try_from(id).ok()) else {
            return;
        };
        match split::part(user) {
            Part::Build => building.push(Rating { user, item, stars }),
            p if p == judge => {
                let j = judged.entry(user).or_insert_with(|| Judged {
                    shown: Vec::new(),
                    held: Vec::new(),
                });
                if split::held_out(user, movie) {
                    j.held.push(item);
                } else {
                    j.shown.push(item);
                }
            }
            _ => {}
        }
    })
    .map_err(missing)?;
    let likes = Likes::from_ratings(building, cf::MIN_LIKES);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let matrix = cf::build(&likes, cf::CHOSEN, TOP, threads);
    let popular: Vec<i64> = likes
        .popularity()
        .into_iter()
        .map(|(id, _)| i64::from(id))
        .collect();
    let in_matrix: HashSet<i64> = popular.iter().copied().collect();

    // The same users the matrix-only number was judged on: enough shown, something held out
    // that the matrix holds. In id order, so `--limit` takes the same users every run.
    let mut users: Vec<(u32, Vec<i64>, Vec<i64>)> = judged
        .into_iter()
        .filter_map(|(user, j)| {
            let held: Vec<i64> = j
                .held
                .into_iter()
                .map(i64::from)
                .filter(|i| in_matrix.contains(i))
                .collect();
            let shown: Vec<i64> = j.shown.into_iter().map(i64::from).collect();
            (shown.len() >= MIN_SHOWN && !held.is_empty()).then_some((user, shown, held))
        })
        .collect();
    users.sort_by_key(|(u, _, _)| *u);
    if let Some(n) = limit {
        users.truncate(n);
    }
    println!(
        "  matrix from {} build users' likes over {} titles; judged on {} users",
        likes.people(),
        likes.items(),
        users.len()
    );

    let settings = if check {
        coverage_settings()
    } else {
        settings(tune)
    };
    let mut tallies: Vec<Tally> = settings.iter().map(|_| Tally::default()).collect();
    let mut popularity = 0.0;
    let mut rank_ms: Vec<f64> = Vec::new();
    let mut sizes: Vec<usize> = Vec::new();
    // Seconds spent per stage over all users: modes, the axes, gathering candidates.
    let mut spent = [0.0f64; 3];
    let started = Instant::now();
    for (_, shown, held) in &users {
        let met: HashSet<i64> = shown.iter().copied().collect();
        let liked: Vec<(i64, f64)> = shown.iter().map(|id| (*id, 1.0)).collect();
        let with_vectors: Vec<(i64, Vec<f32>, f64)> = shown
            .iter()
            .filter_map(|id| {
                let v = semantic.index.vector(*id)?;
                Some((*id, v.iter().map(|x| f32::from(*x)).collect(), 1.0))
            })
            .collect();
        let t = Instant::now();
        let modes = sinephile_taste::modes::modes(&with_vectors);
        spent[0] += t.elapsed().as_secs_f64();
        let mut loved: HashMap<i64, Vec<(i64, f32)>> = HashMap::new();
        let strongest = match asked {
            Some(n) => hybrid::strongest_n(&liked, n),
            None => hybrid::strongest(&liked),
        };
        for from in strongest {
            let Ok(from32) = u32::try_from(from) else {
                continue;
            };
            for &(to, score) in matrix.neighbours(from32) {
                let to = i64::from(to);
                if !met.contains(&to) {
                    loved.entry(to).or_default().push((from, score));
                }
            }
        }
        let t = Instant::now();
        let axes =
            sinephile_taste::axes::axes(&db, &liked, &sinephile_taste::Statements::default())
                .await?;
        spent[1] += t.elapsed().as_secs_f64();
        let t = Instant::now();
        let candidates =
            hybrid::candidates(&db, semantic, &modes, &met, &loved, &axes, &HashSet::new()).await?;
        spent[2] += t.elapsed().as_secs_f64();
        sizes.push(candidates.len());
        let by_id: HashMap<i64, &rank::Candidate> = candidates.iter().map(|c| (c.id, c)).collect();
        let held32: Vec<u32> = held.iter().filter_map(|i| u32::try_from(*i).ok()).collect();

        let top: Vec<u32> = popular
            .iter()
            .filter(|i| !met.contains(i))
            .take(K)
            .filter_map(|i| u32::try_from(*i).ok())
            .collect();
        popularity += recall_of(&top, &held32);

        for (s, tally) in settings.iter().zip(tallies.iter_mut()) {
            let t = Instant::now();
            let picks = rank::rank(&candidates, &s.weights, &caps, s.diversity, K);
            rank_ms.push(t.elapsed().as_secs_f64() * 1000.0);
            let ids: Vec<u32> = picks
                .iter()
                .filter_map(|p| u32::try_from(p.id).ok())
                .collect();
            tally.recall += recall_of(&ids, &held32);
            tally.reached.extend(picks.iter().map(|p| p.id));
            if let Some(d) = intra_list_diversity(&picks, &by_id) {
                tally.ild += d;
                tally.ild_rails += 1;
            }
            if breaks_caps(&picks, &by_id) {
                tally.over_caps += 1;
            }
        }
    }
    let n = users.len().max(1) as f64;
    let base = popularity / n;
    rank_ms.sort_by(f64::total_cmp);
    sizes.sort_unstable();
    let pct = |v: &[f64], p: f64| v[((v.len() as f64 - 1.0) * p) as usize];
    println!(
        "  {} users in {:.0} s; candidates per user: median {}, max {}",
        users.len(),
        started.elapsed().as_secs_f64(),
        sizes[sizes.len() / 2],
        sizes[sizes.len() - 1]
    );
    println!(
        "  time in: modes {:.0} s, axes {:.0} s, gathering candidates {:.0} s",
        spent[0], spent[1], spent[2]
    );
    println!(
        "  ranking one user's candidates: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms (release build, this machine)\n",
        pct(&rank_ms, 0.5),
        pct(&rank_ms, 0.95),
        rank_ms[rank_ms.len() - 1]
    );
    println!("  popularity                                    Recall@{K} {base:.4}");

    let mut best: Option<(f64, usize)> = None;
    let ild_of = |t: &Tally| t.ild / t.ild_rails.max(1) as f64;
    let coverage_of = |t: &Tally| t.reached.len() as f64 / in_matrix.len() as f64;
    for (i, (s, t)) in settings.iter().zip(&tallies).enumerate() {
        let recall = t.recall / n;
        let lift = recall / base - 1.0;
        let coverage = coverage_of(t);
        let ild = ild_of(t);
        println!(
            "  near {:.2} loved {:.2} popular {:.2} diversity {:.2}   Recall@{K} {recall:.4} ({:+.1}%)   coverage {:.1}%   ILD {ild:.3}   over caps {}",
            s.weights.near,
            s.weights.loved,
            s.weights.popular,
            s.diversity,
            lift * 100.0,
            coverage * 100.0,
            t.over_caps
        );
        if coverage > 0.15 && best.is_none_or(|(r, _)| recall > r) {
            best = Some((recall, i));
        }
    }
    if check {
        // The rule, fixed before this ran (docs/eval-results.md): the best recall whose
        // coverage over every tuning user is at least 16%, a point clear of E2's 15%.
        let pick = settings
            .iter()
            .zip(&tallies)
            .filter(|(_, t)| coverage_of(t) >= 0.16)
            .max_by(|a, b| a.1.recall.total_cmp(&b.1.recall));
        match pick {
            Some((s, t)) => println!(
                "\n  best with coverage of at least 16%: near {} loved {} popular {} diversity {} (Recall@{K} {:.4}, coverage {:.1}%)",
                s.weights.near,
                s.weights.loved,
                s.weights.popular,
                s.diversity,
                t.recall / n,
                coverage_of(t) * 100.0
            ),
            None => println!("\n  no setting reaches 16% coverage"),
        }
        return Ok(true);
    }
    if tune {
        if let Some((recall, i)) = best {
            let s = settings[i];
            println!(
                "\n  best on the tuning users with coverage above 15%: near {} loved {} popular {} diversity {} (Recall@{K} {recall:.4})",
                s.weights.near, s.weights.loved, s.weights.popular, s.diversity
            );
        }
        return Ok(true);
    }
    // The app's setting is first; the same by score alone second.
    let (app, plain) = (&tallies[0], &tallies[1]);
    let lift = app.recall / n / base - 1.0;
    let e1 = lift > 0.40;
    let e2 = coverage_of(app) > 0.15;
    let e3 = app.over_caps == 0 && ild_of(app) >= ild_of(plain);
    let said = |ok: bool| if ok { "met" } else { "NOT met" };
    println!(
        "\n  E1, more than 40% above popularity ({:+.1}%): {}",
        lift * 100.0,
        said(e1)
    );
    println!(
        "  E2, coverage above 15% ({:.1}%): {}",
        coverage_of(app) * 100.0,
        said(e2)
    );
    println!(
        "  E3, no rail over a cap ({}) and variety at least the score order's ({:.3} against {:.3}): {}",
        app.over_caps,
        ild_of(app),
        ild_of(plain),
        said(e3)
    );
    Ok(e1 && e2 && e3)
}

/// One profile's "Where your taste points" as Home builds it, each card's reason beside it,
/// and how the reasons divide: what 17.3's look at the real window reads in full (the window
/// shows eight), and what the case study quotes. Run it on a copy (`SINEPHILE_DATA_DIR`):
/// Home keeps the profile's taste model in its cache.
pub async fn rail(data_dir: &Path, profile: i64) -> Result<bool, EvalError> {
    let (mut engine, hybrid) = crate::search::engine_as_app(data_dir)?;
    if !hybrid {
        println!("rail: no meaning index here, and the taste rail needs one");
        return Ok(false);
    }
    let db = sinephile_persistence::Db::open_in(data_dir).await?;
    let cache = sinephile_taste::model::ModelCache::default();
    let started = Instant::now();
    let rails =
        sinephile_taste::home::rails(&db, engine.semantic_mut(), profile, false, None, &cache)
            .await?;
    let took = started.elapsed().as_secs_f64() * 1000.0;
    let Some(taste) = rails.iter().find(|r| r.key == "taste") else {
        println!("rail: profile {profile} has no taste rail");
        return Ok(false);
    };
    let facts: HashMap<i64, (String, Option<i64>)> =
        sinephile_persistence::repositories::MediaRepository::new(&db)
            .facts(&taste.items)
            .await?
            .into_iter()
            .map(|f| (f.id, (f.title, f.year)))
            .collect();
    println!(
        "rail: profile {profile}, \"{}\", Home built in {took:.0} ms\n",
        taste.title
    );
    let (mut loved, mut close, mut widely) = (0, 0, 0);
    for (n, id) in taste.items.iter().enumerate() {
        let note = taste.notes.get(id).map_or("", String::as_str);
        if note.starts_with("Loved by") {
            loved += 1;
        } else if note.starts_with("Close to") {
            close += 1;
        } else {
            widely += 1;
        }
        let (title, year) = facts.get(id).cloned().unwrap_or_default();
        let year = year.map_or(String::new(), |y| format!(" ({y})"));
        println!("  {:>2}. {title}{year}: {note}", n + 1);
    }
    println!(
        "\nreasons: {loved} loved by people who loved, {close} close in meaning, {widely} widely loved"
    );
    Ok(!taste.items.is_empty())
}
