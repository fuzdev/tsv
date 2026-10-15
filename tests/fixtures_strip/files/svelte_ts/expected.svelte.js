export class C {
	a = $state   (1);
	b    = $derived(this.a * 2);
}
export function fn(c   ) {
	let d = $state          (null);
	return d;
}
