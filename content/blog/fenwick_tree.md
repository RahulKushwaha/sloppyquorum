---
title: "Fenwick Tree Or Binary Indexed Tree"
date: 2020-07-07T08:59:16-07:00
slug: ""
description: "It is a sequence of numbers where each element is the sum of the last two number."
keywords: []
draft: true
tags: []
math: false
toc: false
---
### Why?

Suppose we have an array and we want to answer a query: Find sum of all the elements from index i to j.

### Method 1: 
Iterate from i to j and accumulate the result. 
	
	int result = 0;
	for(int a = i; a <= j; a ++){
		result += input[a];
	}

Time Complexity: O(n) [In the worst case we can query Sum(1, n)]

### Method 2: 
We can have an auxiliary array called aux of the same length as the input. 

    aux[i] = sum of all the elements from 1 to i.

Populating the aux array: 

	int currentsum = 0;
	for(int a = 1; a <= n; a ++){
		currentsum += input[a];
		aux[a] = currentsum;
	}

Therefore,

	Sum(i, j) = aux[j] - aux[i - 1];

Time Complexity: Constant
Space Complexity: O(n)

But there is one problem with the above apporach, if we want to change the element at any particular index. In that case we would have to update the whole aux array in the worst case, Change(0, d).

Fenwick tree comes to rescue in this situation. The basic principle behind this approach is that we can divide the sum of a particular segment of the input array in multiple parts.
For example:
 
    seg_sum(i) = Sum(1, r) + Sum(r + 1, s + 1) + ... + Sum(z + 1, i)

But how to find out the segments and how to parition the segment in an efficient way.

Diversion:
Suppose we have an integer, num = 12.
Binary representation, 12 = 1100

Let's define a function, 

    g(x) = { turn off the last set of the number in the binary representation. }
    g(12) = {1000} base 2 = 8

    17 = {10001}
    g(17) = {10000} = 16

So, it clear from the above explanation that if we turn off the last set bit of a number we get a number which is smaller than the original number.

## Range Sum:
 
Let us define another function called fenwick sum,

    fsum(x) = sum(g(x) + 1, x)

Basically what this means is that fenwick sum, fsum, is equal to the sum of all the elements from index, g(x) + 1 to x. Or in other words we can also so that: 

    fsum(x) = input[g(x) + 1] + input[g(x) + 2] + ... + input[x]

For example: 

    fsum(12) = sum of all the elements from 9 to 12.

Therefore to answer the original question, Sum(i, j), which is the sum of all the elements from index i to j we can say:

	Sum(i) = fsum(i) + fsum(g(x)) + fsum(g(g(x))) + ... + fsum(g((....(x))))

The above can be restated in the following way as well.
	
	int result = 0;
	while(x){
		result += fsum(x);
		x = g(x);
	}

	int g(int x){
		// This line of code turns off the last bit of the number.
		x = x - (x & -x);
	}

Now we can easily see that

	Sum(i, j) = Sum(j - 1) - Sum(i)

Time Complexity: The application of function g(n) reduces one bit at time. In worst case scenario the complexity of Sum(n), 1111..1111, will be equal t0 the number of bits, O(log(n)).
To calcuate the sum from i to j we need to call Sum two times. Therefore, the total time complexity is: 2 * O(lg(n)) => O(lg(n))

## Updation:
Suppose we want to update a number at a particular index? Before this we need to understand how a number at a particular index contributes to the different segments.

For example:

	Sum(21) = fsum(21) + fsum(20) + fsum(16)
	Sum(21) = [sum of elements from 21 to 21] + [sum of elements from 17 to 20] + [sum of elements from 1 to 16]

We can see clearly from the above example that element at index 1 never makes contribution to fsum(i) where i >= 17, iff total number of elements is 21. We will see later how the size of the input plays an important role in determining that.
Let's see which all segments 1 makes contribution to: 

	fsum(1) = [sum from 1 to 1]
	fsum(2) = [sum from 1 to 2]
	fsum(3) = [sum from 3 to 3]
	fsum(4) = [sum from 1 to 4]
	fsum(5) = [sum from 5 to 5]
	fsum(6) = [sum from 6 to 5]
	.
	.
	fsum(8) = [sum from 1 to 8]


As we can see the number of contributions made by the element at index 1 is fsum(1), fsun(2), fsum(4), fsum(8), ...
Let us define one more function, h(x)

	int h(int x){
		return x + (x & -x);
	}

More formally we can say that the number of contributions made by a particular number at index i is as follows: 
    
    number of contributions by input[i] = COUNT(fsum(i), fsum(h(x)), fsum(h(h(x))), ... )

There is one point to remember that h(h(h....(x))) <= n, where n is the total number of elements. The total number of times function h(x) can be called is equal to lg(n). Whenever we update any element we need to perform O(lg(n)) operations to update O(lg(n)) segments or O(lg(n)) fsum(x) segments.

    class NumArray {
	public:
	    int * bit;
	    int * input;
	    int length;
	    NumArray(vector<int> &nums) {
	        length = nums.size();
	        
	        if(length == 0){
	            bit = NULL;
	            input = NULL;
	            return;
	        }
	        
	        bit = new int[length + 1];
	        input = new int[length];
	        for(int i = 0; i < length; i ++){
	            bit[i] = 0;
	            input[i] = 0;
	        }
	        
	        bit[length] = 0;
	        
	        for(int i = 0; i < length; i ++){
	            update(i, nums[i]);
	        }
	        
	        for(int i = 0; i < length; i ++){
	            input[i] = nums[i];
	        }
	    }

	    void update(int i, int val) {
	        int diff = val - input[i];
	        input[i] = val;
	        i ++;
	        while(i <= length){
	            bit[i] += diff;
	            i = i + (i & (-i));
	        }
	    }
	    
	    int sum(int i){
	        int total = 0;
	        i ++;
	        while(i != 0){
	            total += bit[i];
	            i = i - (i & (-i));
	        }
	        
	        return total;
	    }

	    int sumRange(int i, int j) {
	        return sum(j) - sum(i - 1);
	    }
	};
