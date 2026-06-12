<script lang="ts">
	interface Empty {  }

	interface Single {  prop:string;  }

	interface Multiple {  a:number;  b:string;  c:boolean;  }

	interface WithMethods {  method1():void;  method2(x:number):string;  }

	interface Optional {  required:string;  optional?:number;  }

	export interface Exported {  prop:string;  }

	interface Callable {  ():void;  n:number;  }

	interface Constructable {  new(x:number):object;  }

	interface WithIndex {  [key:string]:unknown;  }
</script>
