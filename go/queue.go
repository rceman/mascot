package main

// Bounded blocking queue used for provider-to-UI events (64), provider work
// items (16), control commands (16) and control output records (256). A
// blocked producer provides real backpressure; close releases waiters.

import "sync"

type boundedQueue[T any] struct {
	mu       sync.Mutex
	notFull  sync.Cond
	notEmpty sync.Cond
	items    []T
	capacity int
	closed   bool
}

func newBoundedQueue[T any](capacity int) *boundedQueue[T] {
	q := &boundedQueue[T]{capacity: capacity}
	q.notFull.L = &q.mu
	q.notEmpty.L = &q.mu
	return q
}

// push blocks until there is room or the queue closes. Returns false if the
// queue is closed before the value is accepted.
func (q *boundedQueue[T]) push(v T) bool {
	q.mu.Lock()
	for {
		if q.closed {
			q.mu.Unlock()
			return false
		}
		if len(q.items) < q.capacity {
			q.items = append(q.items, v)
			q.mu.Unlock()
			q.notEmpty.Signal()
			return true
		}
		q.notFull.Wait()
	}
}

// tryPush fails when the queue is full or closed.
func (q *boundedQueue[T]) tryPush(v T) bool {
	q.mu.Lock()
	if q.closed || len(q.items) >= q.capacity {
		q.mu.Unlock()
		return false
	}
	q.items = append(q.items, v)
	q.mu.Unlock()
	q.notEmpty.Signal()
	return true
}

func (q *boundedQueue[T]) pop() (T, bool) {
	q.mu.Lock()
	if len(q.items) == 0 {
		q.mu.Unlock()
		var zero T
		return zero, false
	}
	v := q.items[0]
	q.items = q.items[1:]
	if len(q.items) == 0 {
		q.items = nil
	}
	q.mu.Unlock()
	q.notFull.Signal()
	return v, true
}

// waitPop blocks until an item is available or the queue is closed and empty.
func (q *boundedQueue[T]) waitPop() (T, bool) {
	q.mu.Lock()
	for {
		if len(q.items) > 0 {
			v := q.items[0]
			q.items = q.items[1:]
			if len(q.items) == 0 {
				q.items = nil
			}
			q.mu.Unlock()
			q.notFull.Signal()
			return v, true
		}
		if q.closed {
			q.mu.Unlock()
			var zero T
			return zero, false
		}
		q.notEmpty.Wait()
	}
}

func (q *boundedQueue[T]) len() int {
	q.mu.Lock()
	defer q.mu.Unlock()
	return len(q.items)
}

func (q *boundedQueue[T]) cap() int { return q.capacity }

func (q *boundedQueue[T]) close() {
	q.mu.Lock()
	q.closed = true
	q.mu.Unlock()
	q.notFull.Broadcast()
	q.notEmpty.Broadcast()
}
