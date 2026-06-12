// Single unconstrained type param: pure `.ts` adds no trailing comma (unlike `.svelte`'s `<T,>`).

// Short - whole signature fits inline (contrast)
const short=<T>():{a:T;b:T}=>null as any;

// Boundary: signature fits inline at exactly 100 chars - stays on one line
const fitA=<T>():{a:Aaaaaaaa<T>;bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:T}=>null as any;

// Boundary: signature at 101 chars - breaks, <T> stays inline
const fitB=<T>():{a:Aaaaaaaa<T>;bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:T}=>null as any;

// Empty params + breaking object return: <T> stays inline, object expands (corpus: make_deferred)
const make=<T>():{first:Promise<T>;second:(value:T)=>void;third:(reason:Error)=>void}=>null as any;
