export class C {
	a = $state<T>(1);
	b: U = $derived(this.a * 2);
}
export function fn(c: T) {
	let d = $state<T | null>(null);
	return d;
}
