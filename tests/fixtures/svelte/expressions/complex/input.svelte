<div>{a ? b : c}</div>

<div>{a ? (b ? c : d) : e}</div>

<div>{a && b && c}</div>

<div>{a || b || c}</div>

<div>{(a && b) || (c && d)}</div>

<div>{fn(a, b, c)}</div>

<div>{obj.method(a).chain(b).end()}</div>

<div>{arr[0][1][2]}</div>

<div>{obj[key][nested]}</div>

<div>{[a, b, c]}</div>

<div>{{a, b, c}}</div>

<div>{`template ${a} literal ${b}`}</div>

<div>{a + b * c - d / e}</div>

<div>{(a + b) * (c - d)}</div>

<div>{fn(...args)}</div>

<div>{[...a, ...b]}</div>

<div>{{...a, ...b}}</div>

<div>{(() => a + b)()}</div>

<div>{((x) => x * 2)(a)}</div>

<div>{a?.b?.c}</div>

<div>{a?.[b]?.()}</div>

<div>{a ?? b ?? c}</div>

<div>{(a ?? b) || c}</div>

<div>{typeof a}</div>

<div>{!a}</div>

<div>{~a}</div>

<div>{a++}</div>

<div>{++a}</div>

<div>{new Date()}</div>

<div>{new Map([['a', 1]])}</div>

<div>{fn(a, fn2(b, fn3(c)))}</div>

<div>{a === b ? (c === d ? e : f) : g}</div>
