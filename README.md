[![Build Status](https://travis-ci.org/daviddonna/abc-rs.svg?branch=master)](https://travis-ci.org/daviddonna/abc-rs)

# abc-rs
Karaboga's Artificial Bee Colony in Rust (now in parallel!)

[Documentation](https://daviddonna.github.io/abc-rs) is now available.

### The Algorithm

The [Artificial Bee Colony](http://mf.erciyes.edu.tr/abc/) is
an optimization algorithm. It considers a set of solution candidates by
sending conceptual "bees" to work on those solutions. There are three
kinds of bee:

* One **worker** bee is dedicated to each solution. Each time a worker
bee runs, it looks at a solution near the current one, keeping each
improvement.
* **Observer** bees are like workers, but are not dedicated to a single
solution. Instead, they look at all of the active solutions and choose
one randomly to work on. Observers usually prefer to work on
higher-fitness solutions.
* A worker or observer whose solution appears to be a local maximum
becomes a **scout**. Scouting entails generating a new, random solution
to break out of a rut.

### What Can Bees Do For You

The ABC algorithm can be -- and has been -- applied to a variety of
applications. As with many such algorithms, the logic is fairly agnostic
about the domain that it works in. In fact, the prerequisites for using
the algorithm are:

* a data structure with a solution,
* a way of generating new, random solutions,
* a way of generating solutions "near" an existing solution, and
* a fitness function to score solutions.

A solution could be a game-playing AI, a blueprint for a building, or
just a point in space, and the `abc` crate treats them all alike. Simply
implement the [`Context`](https://daviddonna.github.io/abc-rs/abc/trait.Context.html)
trait for a type of your choice, construct a `Hive`, and start running.

### Synchronous and Asynchronous Running

Speaking of running,`abc` supports two run modes:

* running for a [fixed number of rounds](https://daviddonna.github.io/abc-rs/abc/struct.Hive.html#method.run_for_rounds)
and returning the best solution, or
* running [continuously in the background](https://daviddonna.github.io/abc-rs/abc/struct.Hive.html#method.stream)
and sending each improved solution over a Rust channel.

### Parallelism

The `abc` crate takes advantage of Rust's excellent concurrency support
to explore the same space. This means that heavy computation can be
distributed across multiple CPU cores, or I/O-bound evaluation can run
without blocking. The hive maintains a queue of bees, and the threads
each take bees from the queue and apply the bees' logic to the
solutions. So, at pretty much any given moment, there is a different bee
working in each thread.
