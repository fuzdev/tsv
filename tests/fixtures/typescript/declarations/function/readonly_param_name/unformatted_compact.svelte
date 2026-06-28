<script lang="ts">
// `readonly` is a contextual keyword — a valid ordinary parameter name when
// nothing binds after it (it is not a `readonly` modifier here).
function fn1(a,readonly){}

function fn2(readonly:number){}

function fn3(readonly=1){}

function fn4(readonly?:string){}

const arrow=(readonly,a)=>readonly;
</script>
