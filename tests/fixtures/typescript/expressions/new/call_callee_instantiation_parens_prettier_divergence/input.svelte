<script lang="ts"></script>

<!-- a `new` callee that is an instantiation over a call keeps its parens -->
{new (f()<T>)()}
{new (a.b()<T>)()}
{new (f().c<T>)(x)}
{new (f()`x`<T>)()}
{new (f()!<T>)()}
