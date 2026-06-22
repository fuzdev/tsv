<script lang="ts">
	// Named module augmentation
	declare module 'pkg' {
		export function fn(): void;
	}

	// Scoped package
	declare module '@a/b' {
		export const value: string;
	}

	// Wildcard pattern for assets
	declare module '*.svg' {
		const content: string;
		export default content;
	}

	declare module '*.css' {
		const styles: Record<string, string>;
		export default styles;
	}

	declare module '*.json' {
		const value: unknown;
		export default value;
	}

	// Module with multiple exports
	declare module 'multi' {
		export interface A {
			prop: string;
		}
		export function configure(a: A): void;
		export const VERSION: string;
		export default function main(): void;
	}

	// Module with generic exports
	declare module 'generic' {
		export function identity<T>(x: T): T;
		export class Container<T> {
			value: T;
		}
	}

	// Module with nested namespace
	declare module 'nested' {
		namespace Inner {
			interface A {
				prop: string;
			}
		}
		export function fn(a: Inner.A): void;
	}

	// Module with re-exports
	declare module 'reexport' {
		export * from './a';
		export { B, C } from './b';
	}

	// Empty module (type-only augmentation marker)
	declare module 'empty';
</script>
