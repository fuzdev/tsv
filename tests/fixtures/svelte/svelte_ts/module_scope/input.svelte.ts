let a = $state(0);
let b = $derived(a * 2);

$effect(() => {
	console.log(a);
});

export function fn() {
	a++;
}
