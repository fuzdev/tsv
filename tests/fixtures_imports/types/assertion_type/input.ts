let a = b as { [import('./t.js')]: number };
let c = d satisfies { [import('./u.js')]: number };
