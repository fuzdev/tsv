<script lang="ts">
	class C {
		// Basic method - 100 chars (stays inline)
		method(aaaaaa: string,bbbbbb: string,cccccc: string,dddddd: string,eeeeeee: string): void {}

		// Basic method - 101 chars (wraps)
		method(aaaaaa: string,bbbbbb: string,cccccc: string,dddddd: string,eeeeeeee: string,): void {}

		// Async method - 100 chars (stays inline)
		async method(aaa: string,bbb: string,ccc: string,ddd: string,eeee: string): Promise<void> {}

		// Async method - 101 chars (wraps)
		async method(aaa: string,bbb: string,ccc: string,ddd: string,eeeee: string,): Promise<void> {}

		// Static method - 100 chars (stays inline)
		static method(aaa: string,bbb: string,ccc: string,ddd: string,eeeeeeeeeeee: string): void {}

		// Static method - 101 chars (wraps)
		static method(aaa: string,bbb: string,ccc: string,ddd: string,eeeeeeeeeeeee: string,): void {}

		// Generic method - 100 chars (stays inline)
		method<T, U, V>(aaa: T,bbb: U,ccc: V,ddd: string,eee: string,fff: string,ggggggg: T): T {}

		// Generic method - 101 chars (wraps)
		method<T, U, V>(aaa: T,bbb: U,ccc: V,ddd: string,eee: string,fff: string,gggggggg: T,): T {}
	}
</script>
