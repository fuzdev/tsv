<script lang="ts">
// NoInfer in function parameters
function fn1<T>(a:T,b:NoInfer<T>):T{return a;}

// NoInfer in declared function
declare function fn2<T>(values:T[],fallback:NoInfer<T>):T;

// NoInfer in type alias
type Callback<T>=(value:NoInfer<T>)=>void;

// NoInfer nested in complex type
type Box<T>={
value:T;
setter:(val:NoInfer<T>)=>void;
};

// NoInfer with constraints
function fn3<T extends object>(a:T,b:NoInfer<T>):T{return a;}
</script>
