+++
author = "Rahul Kushwaha"
title = "Copy Elision in C++"
date = "2024-02-05"
description = "How to map a Relational Table on a Key-Value Store? "
tags = [
    "c++"
]
summary = "Key-value stores provide a generic API to store and retrieve values using the key. In this post, we will see how relational tables can be mapped to key-value stores. "
+++
C++ has a lot of nice tricks and optimizations but sometimes those tricks can leave you scratching your head. One such instance happened to me while learning about the different constructors and their nuances.

To illustrate the idea with an example, consider the following class:

```
struct Person {
  std::string firstName;
  std::string lastName;

  Person() : firstName("Hello"), lastName("World") {
    std::cout << "Default Constructor invoked." << std::endl;
  }

  Person(Person&& p) noexcept
      : firstName(std::move(p.firstName)), 
        lastName(std::move(p.lastName)) {
    std::cout << "Move Constructor invoked." << std::endl;
  }

  Person(std::string firstName, std::string lastName)
      : firstName(std::move(firstName)), 
        lastName(std::move(lastName)) {
    std::cout << "Parameter Constructor invoked." << std::endl;
  }

  Person(Person& person)
      : firstName(person.firstName), lastName(person.lastName) {
    std::cout << "Copy Constructor invoked." << std::endl;
  }

  Person& operator=(const Person& person) {
    std::cout << "Copy Assignment invoked." << std::endl;
    if (this != &person) {
      this->firstName = person.firstName;
      this->lastName = person.lastName;
    }

    return *this;
  }

  friend std::ostream& operator<<(std::ostream& os, 
    const Person& person){
    os << "firstName: " << person.firstName 
       << " lastName: " << person.lastName;
    return os;
  }
};
```

It is a very simple struct with different constructors(copy, move, etc). I wanted to check my understanding of the different constructors and when is each one of them invoked. Each of the constructors has a side effect of printing on the console. Let’s get started with some fun.

Example #1

```
Person p;
std::cout << p << std::endl;
```
This simple code produces an expected output. Default constructor is invoked.
```
Default Constructor invoked.
firstName: Hello lastName: World
```
Life is good as of now.

Example #2

```
Person p("foo", "bar");
Person p1(p);
cout << p1 << endl;
```
Now we are doing two operations. First, we create an object using the parameterized constructor, and then we use that object to initialize another object using a copy constructor.
```
Parameter Constructor invoked.
Copy Constructor invoked.
firstName: foo lastName: bar
```
The result is as expected.

Example #3
```
Person p("foo", "bar");
Person p1(std::move(p));

cout << p << endl;
cout << p1 << endl;
```
Here we construct the first object using parameterized constructor and the second object using move constructor. We can verify the same in the output as well.
```
Parameter Constructor invoked.
Move Constructor invoked.
firstName:  lastName:
firstName: foo lastName: bar
```
Due to moving the first object printed empty on the console.

Example #4
```
Person p("foo", "bar");
Person p1;
p1 = p;

cout << p << endl;
cout << p1 << endl;
```
Here is a slightly different example. The first object is created using the parameterized constructor. The second object is constructed using the default constructor, and p1 = p invokes a copy assignment.
```
Parameter Constructor invoked.
Default Constructor invoked.
Copy Assignment invoked.
firstName: foo lastName: bar
firstName: foo lastName: bar
```
Till now everything is working as expected.

Example #5

This is the real tricky example that got me confused.
```
Person fun() {
 Person p("foo", "bar");
 return p;
}
~~~
Person p(fun());
std::cout << p << std::endl;
```
In the above case, we should expect that an object gets created on the stack frame of fun using the parameterized constructor and then copied during `Person p(fun())` . But to our surprise, the output looks like something shown below.
```
Parameter Constructor invoked.
firstName: foo lastName: bar
```
Even if we change the above example to the following:
```
Person p = fun();
cout << p << endl;
```
No copy constructor is triggered.

So, what is happening?? Essentially, it is a compiler optimization called Copy Elision which helps in optimizing away copy constructor, copy assignment, etc. Here instead of creating the object on the stack frame of fun , the object is created on the stack frame of the caller, test, and is modified directly by the callee, fun . The compiler is helping in passing the address of the object and making it happen transparently. There is a great lightning talk on this topic by Jon Kalb. Now with the newly acquired knowledge, I was more than excited to verify it. I created the following test to verify.
```
Person fun() {
  Person p("foo", "bar");
  std::cout << "Address of `p` in `fun()`:    " << (&p) << endl;
  return p;
}

void test() {
  Person p = fun();
  std::cout << "Address of `p` in `test()`: " << (&p) << endl;
  cout << p << endl;
}
```
The above code outputs the address of the created object. If the compiler is creating the object at the caller site and the callee is modifying the same object, then the address of both the objects should be the same.

Without further ado, the results:

```
Parameter Constructor invoked.
Address of `p` in `fun()`:  0x7ffee4f33190
Address of `p` in `test()`: 0x7ffee4f33190
firstName: foo lastName: bar
```
Voilà!! The address of both the objects is indeed the same.

There is one more question which is fun to think about. In the above code we are saying the callee is able to modify the object that means it knows the address of the object. But we never pass the address of the object explicitly in our code `Person p(fun())` . Somehow the compiler has to do magic to make it available. I guess this is a topic for another time.

We have just scratched the surface but if you want to read more about the topic:

1. https://en.cppreference.com/w/cpp/language/copy_elision
2. https://www.youtube.com/watch?v=IZbL-RGr_mk&ab_channel=CppCon