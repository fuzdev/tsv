<script lang="ts">
	const a = (A<T>).prop;
	const b = (A<T>).method();
	const c = (A<T, U>).prop;
	const d = (A<T>)?.prop;
	const e = (A<T>)[b];
	const f = obj[A<T>];
	const g = obj[(A<T>).prop];
	const h = (A<T>).a.b;
	const i = (a.b<T>).c;
	const j = (A<T>).a().b;
</script>
