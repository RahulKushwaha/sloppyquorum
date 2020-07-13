---
title: "Fibonacci Numbers"
date: 2020-07-07T08:59:16-07:00
slug: ""
description: " "
keywords: []
draft: true
tags: []
math: false
toc: false
hideReadMore: true
---

### Function definition:

	F(n) = F(n - 1) + F(n - 2)
	F(1) = 0
	F(2) = 1

### Recursive definition:

	int fibo(int n){
		if(n <= 2){
			return 1;
		}

		return fibo(n - 1) + fibo(n - 2)
	}


### Recursion with Memoization:

	unordered_map<int, int> lookup;

	int fibo(int n){
		unordered_map<int, int>::iterator itr = lookup.find(n);
		if(itr != lookup.end()){
			return *(itr->second);
		}

		int result = fibo(n - 1) + fibo(n - 2);
		lookup[n] = result; 
		return result; 
	}


### Iterative method

	int fibo(int n){
		if(n <= 2){
			return 1;
		}

		int a = 1, b = 1, c = 0;
		for(int i = 0; i < n - 2; i ++){
			c = a + b;
			a = b;
			b = c;
		}

		return c;
	}

### Matrix Method
	X = |1 1|
	    |1 0|
	Y = |F(n + 1) F(n)    |
	    |F(n)     F(n - 1)|

	X^n = Y

To multiply the matrix n times we can use binary exponentiation to reduce time complexity to O(lg(n))
