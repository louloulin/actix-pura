#!/usr/bin/env python3
import csv
import random
import sys

# 生成大型测试文件
def generate_large_test_file(filename, num_records=40000):
    first_names = ['John', 'Jane', 'Robert', 'Maria', 'Liu', 'Sato', 'Max', 'Ana', 'Mohamed', 'Lisa',
                  'Carlos', 'Emma', 'Raj', 'Olga', 'Hiroshi', 'Elena', 'David', 'Sofia', 'Ahmed', 'Fatima']
    
    last_names = ['Doe', 'Smith', 'Johnson', 'Garcia', 'Wei', 'Yuki', 'Schmidt', 'Silva', 'Ali', 'Wang',
                 'Rodriguez', 'Brown', 'Patel', 'Ivanov', 'Tanaka', 'Petrova', 'Miller', 'Rossi', 'Khan', 'Kim']
    
    domains = ['example.com', 'test.org', 'mail.net', 'company.co', 'domain.io', 
              'service.com', 'global.net', 'local.org', 'tech.co', 'world.io']
    
    countries = ['USA', 'Canada', 'UK', 'Spain', 'China', 'Japan', 'Germany', 'Brazil', 'Egypt', 
                'Australia', 'India', 'Russia', 'Mexico', 'France', 'Italy', 'South Africa', 'Nigeria', 
                'Argentina', 'Indonesia', 'Turkey']

    print(f"Generating {num_records} test records to {filename}...")
    
    with open(filename, 'w', newline='') as csvfile:
        writer = csv.writer(csvfile)
        
        # 写入标题行
        writer.writerow(['id', 'name', 'email', 'age', 'country'])
        
        # 生成记录
        for i in range(1, num_records + 1):
            first_name = random.choice(first_names)
            last_name = random.choice(last_names)
            name = f"{first_name} {last_name}"
            
            email_username = f"{first_name.lower()}.{last_name[0].lower()}"
            if random.random() < 0.3:  # 30%的概率添加数字
                email_username += str(random.randint(1, 999))
            email = f"{email_username}@{random.choice(domains)}"
            
            age = random.randint(18, 65)
            country = random.choice(countries)
            
            writer.writerow([i, name, email, age, country])
            
            # 显示进度
            if i % 5000 == 0:
                print(f"Generated {i} records...")
    
    print(f"Successfully generated {num_records} records to {filename}")

if __name__ == "__main__":
    filename = "examples/data/large_test.csv"
    num_records = 40000
    
    if len(sys.argv) > 1:
        filename = sys.argv[1]
    if len(sys.argv) > 2:
        num_records = int(sys.argv[2])
        
    generate_large_test_file(filename, num_records) 