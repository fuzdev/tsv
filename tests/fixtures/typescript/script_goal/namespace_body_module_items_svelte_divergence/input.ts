namespace N {
	export const a = 1;
	export namespace Inner {
		export const b = 2;
	}
	@dec
	export class C {}
}

module M {
	export function fn() {}
}

declare namespace D {
	export const c: number;
}

declare module 'a' {
	import x from 'b';
	export function fn(): void;
}
