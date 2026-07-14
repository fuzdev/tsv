// assignment
const a = /** @type {A} */ document.activeElement;
const b = /** @type {A} */ b.parentNode;
const c = /** @satisfies {A} */ expr;

// reassignment
let d;
d = /** @type {A} */ expr;

// assignment target (cast wraps the place being assigned, incl. compound op=)
/** @type {A} */ g.h = expr;
/** @type {A} */ g.h += expr;

// return
function fn1() {
	return /** @type {A} */ expr;
}

// call argument
const e = fn(/** @type {A} */ {});
const f = fn(/** @type {A} */ expr);

// new expression with expand-last arg
new A(/** @type {any} */ a, {});
new A(/** @type {any} */ a, []);
new A(a, /** @type {any} */ b, {});

// unary operand — the wrap is keyed on the comment sitting in the operator→operand gap,
// so it does not care what the operand is: an instantiation expression takes it too
const g1 = !(/** @type {A} */ b.c);
const g2 = !(/** @type {A} */ b<T>);

// default parameter value
function fn2(a = /** @type {A} */ b) {}
function fn3(a, b = /** @type {A} */ c) {}
const fn4 = (a = /** @type {A} */ b) => a;
function fn5({ a = /** @type {A} */ b } = {}) {}
