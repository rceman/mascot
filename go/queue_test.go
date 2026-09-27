package main

import (
	"testing"
	"time"
)

func TestQueuePushPopOrderAndClose(t *testing.T) {
	q := newBoundedQueue[int](2)
	if !q.push(1) || !q.push(2) {
		t.Fatal("push failed")
	}
	if q.tryPush(3) {
		t.Fatal("tryPush should fail when full")
	}
	if q.len() != 2 {
		t.Fatalf("len = %d", q.len())
	}
	if v, ok := q.pop(); !ok || v != 1 {
		t.Fatalf("pop = %v,%v", v, ok)
	}
	q.close()
	if q.push(4) {
		t.Fatal("push after close should fail")
	}
	if v, ok := q.pop(); !ok || v != 2 {
		t.Fatalf("pop = %v,%v", v, ok)
	}
	if _, ok := q.pop(); ok {
		t.Fatal("pop should fail when empty")
	}
}

func TestQueueWaitPopWakesOnClose(t *testing.T) {
	q := newBoundedQueue[int](1)
	done := make(chan bool, 1)
	go func() {
		_, ok := q.waitPop()
		done <- ok
	}()
	time.Sleep(20 * time.Millisecond)
	q.close()
	select {
	case ok := <-done:
		if ok {
			t.Fatal("waitPop should return false on close")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("waitPop did not wake")
	}
}

func TestQueueBlockedProducerWakesOnDrain(t *testing.T) {
	q := newBoundedQueue[int](1)
	q.push(7)
	done := make(chan bool, 1)
	go func() { done <- q.push(9) }()
	time.Sleep(20 * time.Millisecond)
	if v, ok := q.pop(); !ok || v != 7 {
		t.Fatalf("pop = %v,%v", v, ok)
	}
	select {
	case ok := <-done:
		if !ok {
			t.Fatal("push should succeed after drain")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("blocked producer did not wake")
	}
}

func TestQueueBlockedProducerReleasedOnClose(t *testing.T) {
	q := newBoundedQueue[int](1)
	q.push(7)
	done := make(chan bool, 1)
	go func() { done <- q.push(9) }()
	time.Sleep(20 * time.Millisecond)
	q.close()
	select {
	case ok := <-done:
		if ok {
			t.Fatal("push should fail after close")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("blocked producer not released")
	}
}
