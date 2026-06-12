class A {
	a = $state(0);
	b = $derived(this.a * 2);

	constructor() {
		$effect(() => {
			console.log(this.a);
		});
	}

	fn() {
		this.a++;
	}
}

class B<T> {
	value = $state<T | null>(null);

	get doubled() {
		return $derived(this.value);
	}
}
