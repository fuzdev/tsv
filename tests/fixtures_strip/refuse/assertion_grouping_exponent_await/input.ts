async function f(x: Promise<number>) {
	return await x as number ** 2;
}
