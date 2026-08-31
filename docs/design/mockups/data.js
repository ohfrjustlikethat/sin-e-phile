/* Real films, real metadata. SPEC.md §9.0 forbids lorem ipsum and grey placeholder
 * boxes in these mockups, because they hide exactly the problems that need seeing:
 * how chrome behaves against a bright still, whether type holds at real title
 * lengths, whether a rail reads when the artwork is inconsistent.
 *
 * Every still is a frame extracted with FFmpeg from a public-domain film on the
 * Internet Archive — the legal reference source SPEC.md §2.1 names. Synopses and
 * credits are written here rather than fetched, since Phase 4 has not built the
 * catalogue yet.
 *
 * All three takes import this file, so they differ only in typography and layout —
 * which is what is being chosen between.
 */

const FILMS = {
  notld: {
    title: "Night of the Living Dead",
    year: 1968,
    runtime: 96,
    director: "George A. Romero",
    country: "United States",
    language: "English",
    cert: "Not rated",
    genres: ["Horror", "Independent"],
    rating: 7.8,
    spine: 041,
    stills: ["night-of-the-living-dead-0", "night-of-the-living-dead-1", "night-of-the-living-dead-2"],
    synopsis:
      "Seven strangers barricade themselves in a Pennsylvania farmhouse as the recently dead rise and attack the living. Shot for a hundred and fourteen thousand dollars in monochrome, it invented a genre and buried a bleaker verdict inside it: the monsters outside are the least of the problem.",
    cast: ["Duane Jones", "Judith O'Dea", "Karl Hardman", "Marilyn Eastman"],
    tag: "Because you finished three Val Lewton pictures",
  },
  potemkin: {
    title: "Battleship Potemkin",
    year: 1925,
    runtime: 75,
    director: "Sergei Eisenstein",
    country: "Soviet Union",
    language: "Silent, Russian intertitles",
    cert: "Not rated",
    genres: ["Drama", "Silent"],
    rating: 8.0,
    spine: 007,
    stills: ["battleship-potemkin-0", "battleship-potemkin-1", "battleship-potemkin-2"],
    synopsis:
      "The crew of a Tsarist battleship mutinies over maggot-ridden meat, and the rebellion spreads to the city of Odessa. Eisenstein's montage is the film's argument as much as its technique — the Odessa Steps sequence has been quoted, parodied and stolen for a century.",
    cast: ["Aleksandr Antonov", "Vladimir Barsky", "Grigori Aleksandrov"],
    tag: "Your blind spot: Soviet montage",
  },
  scarlet: {
    title: "Scarlet Street",
    year: 1945,
    runtime: 102,
    director: "Fritz Lang",
    country: "United States",
    language: "English",
    cert: "Not rated",
    genres: ["Film noir", "Drama"],
    rating: 7.7,
    spine: 112,
    stills: ["scarlet-street-0", "scarlet-street-1", "scarlet-street-2"],
    synopsis:
      "A meek cashier and Sunday painter falls for a woman who is already spoken for, and who sees exactly what he is worth. Lang's cruellest American film, and the one where the Production Code let a murderer walk free because the punishment he gets is worse.",
    cast: ["Edward G. Robinson", "Joan Bennett", "Dan Duryea"],
    tag: "A stretch — but we think you're ready",
  },
  chien: {
    title: "Un Chien Andalou",
    year: 1929,
    runtime: 21,
    director: "Luis Buñuel",
    country: "France",
    language: "Silent",
    cert: "Not rated",
    genres: ["Surrealist", "Short"],
    rating: 7.5,
    spine: 003,
    stills: ["un-chien-andalou-0", "un-chien-andalou-1", "un-chien-andalou-2"],
    synopsis:
      "Buñuel and Dalí agreed to include no image that admitted a rational explanation. Twenty-one minutes later, cinema had a different set of possibilities. Still the most-imitated opening in film, and still genuinely difficult to watch.",
    cast: ["Pierre Batcheff", "Simone Mareuil", "Luis Buñuel"],
    tag: "Short enough for tonight",
  },
  detour: {
    title: "Detour",
    year: 1945,
    runtime: 68,
    director: "Edgar G. Ulmer",
    country: "United States",
    language: "English",
    cert: "Not rated",
    genres: ["Film noir"],
    rating: 7.2,
    spine: 088,
    stills: ["detour-0", "detour-1", "detour-2"],
    synopsis:
      "A piano player hitchhikes west to meet his girl and makes one bad decision, then another, then several more. Shot in six days on almost nothing, and the poverty is the point — no film has ever looked more like the inside of its narrator's excuses.",
    cast: ["Tom Neal", "Ann Savage", "Claudia Drake"],
    tag: "Beloved by people like you, seen by almost no one",
  },
  friday: {
    title: "His Girl Friday",
    year: 1940,
    runtime: 92,
    director: "Howard Hawks",
    country: "United States",
    language: "English",
    cert: "Not rated",
    genres: ["Comedy", "Screwball"],
    rating: 7.8,
    spine: 026,
    stills: ["his-girl-friday-0", "his-girl-friday-1", "his-girl-friday-2"],
    synopsis:
      "An editor discovers his ex-wife and best reporter is about to remarry and leave the paper, and spends ninety minutes talking her out of it. The dialogue overlaps because Hawks had them start before the other finished — roughly two hundred and forty words a minute.",
    cast: ["Cary Grant", "Rosalind Russell", "Ralph Bellamy"],
    tag: "Because you loved The Front Page",
  },
  general: {
    title: "The General",
    year: 1926,
    runtime: 79,
    director: "Buster Keaton",
    country: "United States",
    language: "Silent",
    cert: "Not rated",
    genres: ["Comedy", "Silent", "Action"],
    rating: 8.1,
    spine: 015,
    stills: ["the-general-0", "the-general-1", "the-general-2"],
    synopsis:
      "A Confederate engineer chases his stolen locomotive, and his stolen fiancée, through enemy lines. Keaton did every stunt and destroyed a real bridge with a real train — the most expensive shot of the silent era, in a film that lost money and is now untouchable.",
    cast: ["Buster Keaton", "Marion Mack", "Glen Cavender"],
    tag: "Canon gap",
  },
  devil: {
    title: "Beat the Devil",
    year: 1953,
    runtime: 89,
    director: "John Huston",
    country: "United Kingdom",
    language: "English",
    cert: "Not rated",
    genres: ["Comedy", "Adventure"],
    rating: 6.2,
    spine: 204,
    stills: ["beat-the-devil-0", "beat-the-devil-1", "beat-the-devil-2"],
    synopsis:
      "Huston and Truman Capote rewrote each day's pages the night before, and it shows in the best way — a thriller that keeps forgetting to be one and becoming a deadpan joke about its own genre instead. It flopped, then became a cult object.",
    cast: ["Humphrey Bogart", "Jennifer Jones", "Gina Lollobrigida"],
    tag: "Deep cut",
  },
};

/** The one with no artwork — ADR-0013's typographic state, which must be beautiful. */
const NO_ARTWORK = {
  title: "The Passion of Joan of Arc",
  year: 1928,
  runtime: 110,
  director: "Carl Th. Dreyer",
  country: "France",
  language: "Silent",
  genres: ["Drama", "Silent"],
  spine: 062,
  tag: "No artwork — typographic card",
};

const RAILS = [
  { label: "Continue watching", intent: "resume", keys: ["notld", "scarlet", "detour"] },
  { label: "Because you finished three Val Lewton pictures", intent: "safe", keys: ["potemkin", "friday", "chien", "general"] },
  { label: "Your blind spot — Soviet montage", intent: "blindspot", keys: ["potemkin", "general", "devil"] },
  { label: "Beloved by people like you, seen by almost no one", intent: "deepcut", keys: ["detour", "devil", "chien"] },
];

const still = (name) => `./assets/${name}.jpg`;
const mins = (n) => `${Math.floor(n / 60)}h ${n % 60}m`;
