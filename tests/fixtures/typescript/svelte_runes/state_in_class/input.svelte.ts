// Test .svelte.ts file with Svelte 5 runes
export class Counter {
	count = $state(0);
	double = $derived(this.count * 2);

	increment() {
		this.count++;
	}
}
