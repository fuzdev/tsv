function f(a?: Promise<unknown>): void;
function f(a = import('./a.js')) {}
