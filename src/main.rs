use crate::analyser::Analyser;

mod lexer;
mod analyser;

fn main() {
    let mut lexer = lexer::Lexer::new( r#"
struct Point {
    int x;
    int y;
};

struct Student {
    char name[32];
    int age;
    double grade;
};

struct Classroom {
    struct Student students[10];
    int count;
};

int globalCounter;
double globalAverage;
char globalFlag;

double add(double a, double b) {
    double result;
    result = a + b;

    {
        double resultCopy;
        resultCopy = result;
    }

    {
        int temp;
        temp = 10;
    }

    return result;
}

int sumArray(int values[10], int n) {
    int i;
    int sum;

    i = 0;
    sum = 0;

    while (i < n) {
        sum = sum + values[i];
        i = i + 1;
    }

    return sum;
}

double computeAverage(int values[10], int n) {
    int total;
    double avg;

    total = sumArray(values, n);
    avg = total / n;

    return avg;
}

int main() {
    struct Point p;
    struct Student s;
    struct Classroom classroom;

    int numbers[10];
    int i;
    int total;
    double avg;
    char c;

    p.x = 10;
    p.y = 20;

    s.age = 21;
    s.grade = 9.75;
    s.name[0] = 'A';
    s.name[1] = 'n';
    s.name[2] = 'a';

    classroom.count = 1;
    classroom.students[0].age = s.age;
    classroom.students[0].grade = s.grade;

    i = 0;

    while (i < 10) {
        numbers[i] = i + 1;
        i = i + 1;
    }

    total = sumArray(numbers, 10);
    avg = computeAverage(numbers, 10);

    if (avg > 5.0 && total != 0) {
        int passed;
        passed = 1;

        {
            int innerPassed;
            innerPassed = passed + 1;
        }
    } else {
        int failed;
        failed = 1;
    }

    for (i = 0; i < 10; i = i + 1) {
        int localValue;
        localValue = numbers[i];

        {
            int doubled;
            doubled = localValue * 2;
        }
    }

    c = 'z';
    globalCounter = total;
    globalAverage = avg;
    globalFlag = c;

    return 0;
}
"#,);

    let tokens = lexer.get_tokens();

    let mut analyser = Analyser::new(tokens);

    analyser.parse();
    analyser.print_symbols();
}